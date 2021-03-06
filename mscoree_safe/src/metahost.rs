use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;
use std::ptr;
use std::rc::{Rc, Weak};
use std::string::ToString;

use winapi::shared::guiddef::{REFCLSID, REFIID};
use winapi::shared::minwindef::{BOOL, DWORD, LPVOID, ULONG};
use winapi::shared::ntdef::HANDLE;
use winapi::shared::winerror::S_OK;

use winapi::um::objidlbase::{IEnumUnknown};
use winapi::um::unknwnbase::IUnknown;

use mscorlib_safe::BString;

use mscoree_sys::metahost::{CLSID_CLRMetaHost, CLRCreateInstance, ICLRMetaHost, ICLRRuntimeInfo, IID_ICLRMetaHost, IID_ICLRRuntimeInfo};
use mscoree_sys::mscoree::{
    CLSID_TypeNameFactory, 
    CLSID_CLRRuntimeHost, 
    CLSID_CorRuntimeHost, 
    ICLRRuntimeHost, 
    ICorRuntimeHost,
    ITypeNameFactory,
    IID_ICLRRuntimeHost, 
    IID_ICorRuntimeHost, 
    IID_ITypeNameFactory
};

extern "system" {
    pub fn GetCurrentProcess() -> HANDLE;
}

macro_rules! ENUM_CONSTANTS { 
    ($const_type:ty, $(#[$attrs:meta])* enum $name:ident { $($disc:ident = $value:expr),*} ) => {
        $(#[$attrs])*
        pub enum $name {
            $($disc),*
            ,Unknown(String)
        }

        impl ToString for $name {
            fn to_string(&self) -> String {
                match self {
                    $(
                        $name::$disc => String::from($value)
                    ),*
                    , $name::Unknown(v) => v.clone()
                }
            }
        }

        impl From<$const_type> for $name {
            fn from(in_str: $const_type) -> $name {
                match in_str.as_ref() {
                    $(
                        $value => $name::$disc,
                    )*
                    _ => $name::Unknown(in_str)
                }
            }
        }
    };
}

ENUM_CONSTANTS!{String, 
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Hash)]
enum RuntimeVersion {
    V2 = "v2.0.50727", 
    V3 = "v3.0", 
    V4 = "v4.0.30319"
}}

/*CLSID_CorMetaDataDispenser	IID_IMetaDataDispenser, IID_IMetaDataDispenserEx
CLSID_CorMetaDataDispenserRuntime	IID_IMetaDataDispenser, IID_IMetaDataDispenserEx
CLSID_CorRuntimeHost	IID_ICorRuntimeHost
CLSID_CLRRuntimeHost	IID_ICLRRuntimeHost
CLSID_TypeNameFactory	IID_ITypeNameFactory
CLSID_CLRDebuggingLegacy	IID_ICorDebug
CLSID_CLRStrongName	IID_ICLRStrongName*/
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Hash)]
pub enum SupportedInterfaces {
    CorRuntimeHost,
    CLRRuntimeHost, 
    TypeNameFactory, 
}

impl SupportedInterfaces {
    pub fn clsid(&self) -> REFCLSID {
        match self {
            SupportedInterfaces::CorRuntimeHost => &CLSID_CorRuntimeHost,
            SupportedInterfaces::CLRRuntimeHost => &CLSID_CLRRuntimeHost, 
            SupportedInterfaces::TypeNameFactory => &CLSID_TypeNameFactory
        }
    }

    pub fn iid(&self) -> REFIID {
        match self {
            SupportedInterfaces::CorRuntimeHost => &IID_ICorRuntimeHost,
            SupportedInterfaces::CLRRuntimeHost => &IID_ICLRRuntimeHost, 
            SupportedInterfaces::TypeNameFactory => &IID_ITypeNameFactory
        }
    }
}

pub struct IntfCtr {
    inner: *mut LPVOID, 
    intf_ty: SupportedInterfaces
}

pub trait RuntimeInfo {
    fn version(&mut self) -> RuntimeVersion;
    fn loaded(&mut self) -> bool;
    fn loadable(&mut self) -> bool;
    fn started(&mut self) -> bool;
    fn load_library(&mut self, dll_name: &str);
    fn interface(&mut self, supported_intf: SupportedInterfaces) -> IntfCtr;
}

impl Debug for RuntimeInfo + 'static {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "dyn RuntimeInfo{{/*fields omitted*/}}")
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct RuntimeInfoImpl {
    version: RuntimeVersion,
    inner: *mut ICLRRuntimeInfo,
    loaded: Option<bool>, 
    loadable: Option<bool>,
    started: Option<bool>,
}

impl RuntimeInfoImpl {
    fn version(in_ptr: *mut ICLRRuntimeInfo) -> RuntimeVersion {
        assert!(!in_ptr.is_null());
        let mut dw: DWORD = 0;
        let _hr = unsafe {
            (*in_ptr).GetVersionString(ptr::null_mut(), &mut dw)
        };
        //dw now contains length of required buffer
        let mut buffer: Vec<u16> = Vec::with_capacity(dw as usize);
        let hr = unsafe {
            (*in_ptr).GetVersionString(buffer.as_mut_ptr(), &mut dw)
        };

        if hr == S_OK {
            let bs = BString::from_vec(buffer);
            return RuntimeVersion::from(bs.to_string());
        }
        RuntimeVersion::Unknown(String::from(""))
    }
}

impl RuntimeInfo for RuntimeInfoImpl {
    fn version(&mut self) -> RuntimeVersion {
        match self.version {
            RuntimeVersion::V2 | RuntimeVersion::V3 | RuntimeVersion::V4 => return self.version.clone(), 
            RuntimeVersion::Unknown(_) => {}
        }

        let mut dw: DWORD = 0;
        let _hr = unsafe {
            (*self.inner).GetVersionString(ptr::null_mut(), &mut dw)
        };
        //dw now contains length of required buffer
        let mut buffer: Vec<u16> = Vec::with_capacity(dw as usize);
        let hr = unsafe {
            (*self.inner).GetVersionString(buffer.as_mut_ptr(), &mut dw)
        };

        if hr == S_OK {
            let bs = BString::from_vec(buffer);
            self.version = RuntimeVersion::from(bs.to_string());
        }
        self.version.clone()
    }

    fn loaded(&mut self) -> bool {
        match self.loaded {
            Some(b) => return b, 
            None => {}
        }
        let handle = unsafe {GetCurrentProcess()};
        let mut vb: BOOL = 0;
        let _hr = unsafe {(*self.inner).IsLoaded(handle, &mut vb as *mut BOOL)};
        self.loaded = Some(vb < 0);
        vb < 0
    }

    fn load_library(&mut self, dll_name: &str) {

    }

    fn interface(&mut self, supported_intf: SupportedInterfaces) -> IntfCtr {
        let pp_unk: *mut LPVOID = match supported_intf {
            SupportedInterfaces::CLRRuntimeHost => {
                let mut p: *mut ICLRRuntimeHost = ptr::null_mut();
                &mut p as *mut _ as *mut LPVOID
            }, 
            SupportedInterfaces::CorRuntimeHost => {
                let mut p: *mut ICorRuntimeHost = ptr::null_mut();
                &mut p as *mut _ as *mut LPVOID
            }, 
            SupportedInterfaces::TypeNameFactory => {
                let mut p: *mut ITypeNameFactory = ptr::null_mut();
                &mut p as *mut _ as *mut LPVOID
            }
        };
        let _hr = unsafe {
            (*self.inner).GetInterface(supported_intf.clsid(), supported_intf.iid(), pp_unk)
        };
        IntfCtr {inner: pp_unk, intf_ty: supported_intf}
    }

    fn loadable(&mut self) -> bool {
        match self.loadable {
            Some(b) => return b, 
            None => {}
        }
        let mut vb: BOOL = 0;
        let _hr = unsafe {(*self.inner).IsLoadable(&mut vb as *mut BOOL)};
        self.loadable = Some(vb < 0);
        vb < 0
    }

    fn started(&mut self) -> bool {
        match self.started {
            Some(b) => return b,
            None => {}
        }
        let mut vb: BOOL = 0;
        let _hr = unsafe {(*self.inner).IsStarted(&mut vb as *mut BOOL, &mut 0)};
        self.started = Some(vb < 0);
        vb < 0
    }
}

pub trait MetaHost {
    fn runtime(&mut self, version: RuntimeVersion) -> Weak<dyn RuntimeInfo>;
    fn runtimes(&mut self) -> HashMap<RuntimeVersion, Weak<dyn RuntimeInfo>>;
    fn loaded_runtimes(&mut self) -> HashMap<RuntimeVersion, bool>;
}

#[derive(Clone, Debug)]
pub struct MetaHostImpl {
    inner: *mut ICLRMetaHost,
    runtimes: HashMap<RuntimeVersion, Rc<dyn RuntimeInfo>>,
    loaded_runtimes: HashMap<RuntimeVersion, bool>,
}

impl MetaHostImpl {
    fn new() -> Box<MetaHost> {
        let mut mh_ptr: *mut ICLRMetaHost = ptr::null_mut();
        let hr = unsafe {
            CLRCreateInstance(&CLSID_CLRMetaHost, &IID_ICLRMetaHost, &mut mh_ptr as *mut _ as *mut LPVOID)
        };
        if hr == 0 && !mh_ptr.is_null() {
            Box::new(MetaHostImpl {
                inner: mh_ptr, 
                runtimes: HashMap::new(), 
                loaded_runtimes: HashMap::new()
            })
        }
        else {
            panic!("HR = 0x{:x}", hr);
        }
    }
}

impl MetaHost for MetaHostImpl {
    fn runtime(&mut self, version: RuntimeVersion) -> Weak<dyn RuntimeInfo> {
        match self.runtimes.get(&version) {
            Some(ri) => return Rc::downgrade(ri),
            None => {}
        }
        let bs = BString::from_str(&version.to_string());
        let mut ri_ptr: *mut ICLRRuntimeInfo = ptr::null_mut();
        let hr = unsafe {
            (*self.inner).GetRuntime(bs.as_sys(), &IID_ICLRRuntimeInfo, &mut ri_ptr as *mut _ as *mut LPVOID)
        };
        if hr == 0 && !ri_ptr.is_null() {
            let ri = RuntimeInfoImpl {
                version: version.clone(), 
                inner: ri_ptr, 
                loaded: None, 
                loadable: None, 
                started: None };
            let strong = Rc::new(ri);
            let w = Rc::downgrade(&strong);
            self.runtimes.insert(version, strong);
            w
        }
        else {
            panic!("HR = 0x{:x}", hr);
        }
    }

    fn runtimes(&mut self) -> HashMap<RuntimeVersion, Weak<dyn RuntimeInfo>> {
        if self.runtimes.is_empty() {
            let mut ieu_ptr: *mut IEnumUnknown = ptr::null_mut();
            let hr = unsafe {
                (*self.inner).EnumerateInstalledRuntimes(&mut ieu_ptr as *mut *mut IEnumUnknown)
            };
            if hr == 0 && !ieu_ptr.is_null() {
                let mut next_hr = S_OK;
                let mut hmri: HashMap<RuntimeVersion, Rc<dyn RuntimeInfo>> = HashMap::new();
                while next_hr == S_OK {
                    let mut iu_ptr: *mut IUnknown = ptr::null_mut();
                    let mut cfetched: ULONG = 0;
                    next_hr = unsafe {
                        (*ieu_ptr).Next(1, &mut iu_ptr as *mut *mut IUnknown, &mut cfetched as *mut ULONG)
                    };
                    if next_hr == S_OK {
                        let mut ri_ptr: *mut ICLRRuntimeInfo = ptr::null_mut();
                        let inner_hr = unsafe { (*iu_ptr).QueryInterface(&IID_ICLRRuntimeInfo, &mut ri_ptr as *mut _ as *mut LPVOID )};
                        if inner_hr == S_OK && !ri_ptr.is_null() {
                            let mut ri = RuntimeInfoImpl { 
                                version: RuntimeVersion::Unknown(String::from("")), 
                                inner: ri_ptr, 
                                loaded: None, 
                                loadable: None,
                                started: None };
                            let v = ri.version();
                            hmri.insert(v, Rc::new(ri));
                        }
                    }
                }
                self.runtimes = hmri;
            }
        }
        let mut weak_map = HashMap::new();
        self.runtimes.iter().for_each(|(key, value)| {
            weak_map.insert(key.clone(), Rc::downgrade(&value));
        });
        weak_map
    }

    fn loaded_runtimes(&mut self) -> HashMap<RuntimeVersion, bool> {
        if self.loaded_runtimes.is_empty() {
            let mut ieu_ptr: *mut IEnumUnknown = ptr::null_mut();
            let hr = unsafe {
                let handle = GetCurrentProcess();
                (*self.inner).EnumerateLoadedRuntimes(handle, &mut ieu_ptr as *mut *mut IEnumUnknown)
            };
            if hr == 0 && !ieu_ptr.is_null() {
                let mut next_hr = S_OK;
                let mut hmri: HashMap<RuntimeVersion, bool> = HashMap::new();
                while next_hr == S_OK {
                    let mut iu_ptr: *mut IUnknown = ptr::null_mut();
                    let mut cfetched: ULONG = 0;
                    next_hr = unsafe {
                        (*ieu_ptr).Next(1, &mut iu_ptr as *mut *mut IUnknown, &mut cfetched as *mut ULONG)
                    };
                    if next_hr == S_OK {
                        let mut ri_ptr: *mut ICLRRuntimeInfo = ptr::null_mut();
                        let inner_hr = unsafe { (*iu_ptr).QueryInterface(&IID_ICLRRuntimeInfo, &mut ri_ptr as *mut _ as *mut LPVOID )};
                        if inner_hr == S_OK && !ri_ptr.is_null() {
                            let v = RuntimeInfoImpl::version(ri_ptr);
                            hmri.insert(v, true);
                        }
                    }
                }
                self.runtimes().iter().for_each(|(key, _value)|{
                    if !hmri.contains_key(key) {
                        hmri.insert(key.clone(), false);
                    }
                });
                self.loaded_runtimes = hmri;
            }
        }
        let mut clone = HashMap::new();
        self.loaded_runtimes.iter().for_each(|(key, value)|{
            clone.insert(key.clone(), *value);
        });
        clone
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn versions() {
        assert_eq!(RuntimeVersion::V2.to_string(), String::from("v2.0.50727") );
        assert_eq!(RuntimeVersion::V3.to_string(), String::from("v3.0") );
        assert_eq!(RuntimeVersion::V4.to_string(), String::from("v4.0.30319") );
    }
}