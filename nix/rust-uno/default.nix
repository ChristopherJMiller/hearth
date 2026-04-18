# nix/rust-uno/default.nix — Extract and patch the rust_uno crate from LibreOffice 26.2 source
#
# The rust_uno crate provides Rust FFI bindings for the LibreOffice UNO API.
# It lives inside the LO source tree and requires:
#   1. The static source files (ffi/, core/) — extracted from the source tarball
#   2. The generated module (generated/) — produced by 'rustmaker' during the LO build
#
# Since rustmaker requires a full LO build to run, we provide hand-written stubs
# for the specific UNO interfaces hearth-office needs (XDispatch, XDispatchProvider,
# XModel, XFrame). These stubs match the generated API surface and will be replaced
# by real rustmaker output once the full libreoffice-hearth build pipeline is complete.
#
# The crate is patched from crate-type=["cdylib"] to ["lib"] so it can be used
# as a regular Rust dependency.

{ pkgs
, libreoffice-src ? import ../libreoffice-hearth/main.nix { inherit (pkgs) fetchurl fetchgit; }
}:

pkgs.runCommand "rust-uno-crate" {
  nativeBuildInputs = [ pkgs.gnutar pkgs.xz ];
} ''
  # Extract the rust_uno directory from the LO source tarball
  tar -xf ${libreoffice-src} --strip-components=1 --wildcards 'libreoffice-*/rust_uno/'

  mkdir -p $out

  # Copy the crate source
  cp -r rust_uno/* $out/

  # Patch Cargo.toml: change crate-type from cdylib to lib so it can be
  # used as a regular Rust dependency by hearth-office
  sed -i 's/crate-type = \["cdylib"\]/crate-type = ["lib"]/' $out/Cargo.toml

  # The generated/ module is normally created by rustmaker during the LO build.
  # We provide stubs for the interfaces hearth-office uses. These match the
  # generated API surface (opaque pointer wrappers with from_ptr/as_ptr methods).
  mkdir -p $out/src/generated/rustmaker/com/sun/star/frame
  mkdir -p $out/src/generated/rustmaker/com/sun/star/text
  mkdir -p $out/src/generated/rustmaker/com/sun/star/uno
  mkdir -p $out/src/generated/rustmaker/com/sun/star/beans
  mkdir -p $out/src/generated/rustmaker/com/sun/star/util

  # Root mod.rs for generated module
  # Provides both paths: generated::com:: (used by uno_wrapper.rs)
  # and generated::rustmaker::com:: (used by examples and extensions)
  cat > $out/src/generated/mod.rs << 'GENEOF'
#[allow(non_snake_case, non_camel_case_types, unused)]
pub mod rustmaker {
    pub mod com {
        pub mod sun {
            pub mod star {
                pub mod frame;
                pub mod text;
                pub mod uno;
                pub mod beans;
                pub mod util;
            }
        }
    }
}

// Re-export so generated::com:: also works (used by core/uno_wrapper.rs)
pub use rustmaker::com;
GENEOF

  # UNO interface stubs — each interface is an opaque pointer wrapper
  # matching the rustmaker-generated API: from_ptr(), as_ptr(), and typed methods

  cat > $out/src/generated/rustmaker/com/sun/star/uno/mod.rs << 'GENEOF'
pub mod XComponentContext;
pub mod XInterface;
GENEOF

  cat > $out/src/generated/rustmaker/com/sun/star/uno/XComponentContext.rs << 'GENEOF'
use std::ffi::c_void;

pub struct XComponentContext {
    ptr: *mut c_void,
}

impl XComponentContext {
    pub fn from_ptr(ptr: *mut c_void) -> Option<Self> {
        if ptr.is_null() { None } else { Some(Self { ptr }) }
    }
    pub fn as_ptr(&self) -> *mut c_void { self.ptr }
}
GENEOF

  cat > $out/src/generated/rustmaker/com/sun/star/uno/XInterface.rs << 'GENEOF'
use std::ffi::c_void;

pub struct XInterface {
    ptr: *mut c_void,
}

impl XInterface {
    pub fn from_ptr(ptr: *mut c_void) -> Option<Self> {
        if ptr.is_null() { None } else { Some(Self { ptr }) }
    }
    pub fn as_ptr(&self) -> *mut c_void { self.ptr }
}
GENEOF

  cat > $out/src/generated/rustmaker/com/sun/star/frame/mod.rs << 'GENEOF'
pub mod Desktop;
pub mod XDispatch;
pub mod XDispatchProvider;
pub mod XFrame;
pub mod XModel;
pub mod XComponentLoader;
pub mod XController;
pub mod XStatusbarController;
GENEOF

  cat > $out/src/generated/rustmaker/com/sun/star/frame/Desktop.rs << 'GENEOF'
use std::ffi::c_void;

pub struct Desktop {
    ptr: *mut c_void,
}

impl Desktop {
    pub fn create(context: *mut c_void) -> Option<Self> {
        // In real generated code, this calls the UNO service factory
        if context.is_null() { None } else { Some(Self { ptr: context }) }
    }
    pub fn from_ptr(ptr: *mut c_void) -> Option<Self> {
        if ptr.is_null() { None } else { Some(Self { ptr }) }
    }
    pub fn as_ptr(&self) -> *mut c_void { self.ptr }
}
GENEOF

  cat > $out/src/generated/rustmaker/com/sun/star/frame/XDispatch.rs << 'GENEOF'
use std::ffi::c_void;

pub struct XDispatch {
    ptr: *mut c_void,
}

impl XDispatch {
    pub fn from_ptr(ptr: *mut c_void) -> Option<Self> {
        if ptr.is_null() { None } else { Some(Self { ptr }) }
    }
    pub fn as_ptr(&self) -> *mut c_void { self.ptr }

    /// Dispatch a command URL with arguments.
    pub fn dispatch(&self, _url: *mut c_void, _args: *mut c_void) {
        // Stub: in generated code, this calls the C++ bridge
    }
}
GENEOF

  cat > $out/src/generated/rustmaker/com/sun/star/frame/XDispatchProvider.rs << 'GENEOF'
use std::ffi::c_void;
use super::XDispatch::XDispatch;

pub struct XDispatchProvider {
    ptr: *mut c_void,
}

impl XDispatchProvider {
    pub fn from_ptr(ptr: *mut c_void) -> Option<Self> {
        if ptr.is_null() { None } else { Some(Self { ptr }) }
    }
    pub fn as_ptr(&self) -> *mut c_void { self.ptr }

    /// Query for a dispatch handler for the given URL.
    pub fn queryDispatch(
        &self,
        _url: *mut c_void,
        _target_frame_name: *mut c_void,
        _search_flags: i32,
    ) -> Option<XDispatch> {
        // Stub: in generated code, calls C++ bridge
        None
    }
}
GENEOF

  cat > $out/src/generated/rustmaker/com/sun/star/frame/XFrame.rs << 'GENEOF'
use std::ffi::c_void;
use super::XController::XController;

pub struct XFrame {
    ptr: *mut c_void,
}

impl XFrame {
    pub fn from_ptr(ptr: *mut c_void) -> Option<Self> {
        if ptr.is_null() { None } else { Some(Self { ptr }) }
    }
    pub fn as_ptr(&self) -> *mut c_void { self.ptr }

    pub fn getController(&self) -> Option<XController> {
        // Stub
        None
    }
}
GENEOF

  cat > $out/src/generated/rustmaker/com/sun/star/frame/XModel.rs << 'GENEOF'
use std::ffi::c_void;
use crate::core::OUString;

pub struct XModel {
    ptr: *mut c_void,
}

impl XModel {
    pub fn from_ptr(ptr: *mut c_void) -> Option<Self> {
        if ptr.is_null() { None } else { Some(Self { ptr }) }
    }
    pub fn as_ptr(&self) -> *mut c_void { self.ptr }

    /// Get the document URL.
    pub fn getURL(&self) -> OUString {
        // Stub: in generated code, calls C++ bridge to get the document URL
        OUString::from("")
    }
}
GENEOF

  cat > $out/src/generated/rustmaker/com/sun/star/frame/XComponentLoader.rs << 'GENEOF'
use std::ffi::c_void;
use super::super::uno::XInterface::XInterface;
use crate::core::OUString;

pub struct XComponentLoader {
    ptr: *mut c_void,
}

impl XComponentLoader {
    pub fn from_ptr(ptr: *mut c_void) -> Option<Self> {
        if ptr.is_null() { None } else { Some(Self { ptr }) }
    }
    pub fn as_ptr(&self) -> *mut c_void { self.ptr }

    pub fn loadComponentFromURL(
        &self,
        _url: OUString,
        _target: OUString,
        _search_flags: i32,
        _args: *mut c_void,
    ) -> Option<XInterface> {
        // Stub
        None
    }
}
GENEOF

  cat > $out/src/generated/rustmaker/com/sun/star/frame/XController.rs << 'GENEOF'
use std::ffi::c_void;
use super::XModel::XModel;

pub struct XController {
    ptr: *mut c_void,
}

impl XController {
    pub fn from_ptr(ptr: *mut c_void) -> Option<Self> {
        if ptr.is_null() { None } else { Some(Self { ptr }) }
    }
    pub fn as_ptr(&self) -> *mut c_void { self.ptr }

    pub fn getModel(&self) -> Option<XModel> {
        // Stub
        None
    }
}
GENEOF

  cat > $out/src/generated/rustmaker/com/sun/star/frame/XStatusbarController.rs << 'GENEOF'
use std::ffi::c_void;

pub struct XStatusbarController {
    ptr: *mut c_void,
}

impl XStatusbarController {
    pub fn from_ptr(ptr: *mut c_void) -> Option<Self> {
        if ptr.is_null() { None } else { Some(Self { ptr }) }
    }
    pub fn as_ptr(&self) -> *mut c_void { self.ptr }
}
GENEOF

  # text module stubs
  cat > $out/src/generated/rustmaker/com/sun/star/text/mod.rs << 'GENEOF'
pub mod XTextDocument;
pub mod XSimpleText;
pub mod XTextRange;
GENEOF

  cat > $out/src/generated/rustmaker/com/sun/star/text/XTextDocument.rs << 'GENEOF'
use std::ffi::c_void;

pub struct XTextDocument { ptr: *mut c_void }
impl XTextDocument {
    pub fn from_ptr(ptr: *mut c_void) -> Option<Self> {
        if ptr.is_null() { None } else { Some(Self { ptr }) }
    }
    pub fn as_ptr(&self) -> *mut c_void { self.ptr }
    pub fn getText(&self) -> Option<super::XSimpleText::XSimpleText> { None }
}
GENEOF

  cat > $out/src/generated/rustmaker/com/sun/star/text/XSimpleText.rs << 'GENEOF'
use std::ffi::c_void;
use crate::core::OUString;

pub struct XSimpleText { ptr: *mut c_void }
impl XSimpleText {
    pub fn from_ptr(ptr: *mut c_void) -> Option<Self> {
        if ptr.is_null() { None } else { Some(Self { ptr }) }
    }
    pub fn as_ptr(&self) -> *mut c_void { self.ptr }
    pub fn createTextCursor(&self) -> Option<super::XTextRange::XTextRange> { None }
    pub fn insertString(&self, _range: super::XTextRange::XTextRange, _text: OUString, _absorb: u8) {}
}
GENEOF

  cat > $out/src/generated/rustmaker/com/sun/star/text/XTextRange.rs << 'GENEOF'
use std::ffi::c_void;

pub struct XTextRange { ptr: *mut c_void }
impl XTextRange {
    pub fn from_ptr(ptr: *mut c_void) -> Option<Self> {
        if ptr.is_null() { None } else { Some(Self { ptr }) }
    }
    pub fn as_ptr(&self) -> *mut c_void { self.ptr }
}
GENEOF

  # beans module
  cat > $out/src/generated/rustmaker/com/sun/star/beans/mod.rs << 'GENEOF'
pub mod PropertyValue;
GENEOF

  cat > $out/src/generated/rustmaker/com/sun/star/beans/PropertyValue.rs << 'GENEOF'
use std::ffi::c_void;

pub struct PropertyValue { ptr: *mut c_void }
impl PropertyValue {
    pub fn from_ptr(ptr: *mut c_void) -> Option<Self> {
        if ptr.is_null() { None } else { Some(Self { ptr }) }
    }
    pub fn as_ptr(&self) -> *mut c_void { self.ptr }
}
GENEOF

  # util module
  cat > $out/src/generated/rustmaker/com/sun/star/util/mod.rs << 'GENEOF'
pub mod URL;
GENEOF

  cat > $out/src/generated/rustmaker/com/sun/star/util/URL.rs << 'GENEOF'
use std::ffi::c_void;

pub struct URL { ptr: *mut c_void }
impl URL {
    pub fn from_ptr(ptr: *mut c_void) -> Option<Self> {
        if ptr.is_null() { None } else { Some(Self { ptr }) }
    }
    pub fn as_ptr(&self) -> *mut c_void { self.ptr }
}
GENEOF

  # Disable the build.rs link directives when building outside the LO tree.
  # hearth-office handles its own linking via Nix buildInputs.
  cat > $out/build.rs << 'BUILDEOF'
fn main() {
    // Link against UNO libraries when INSTDIR is set (Nix build or LO tree).
    if let Ok(instdir) = std::env::var("INSTDIR") {
        println!("cargo:rustc-link-search=native={instdir}/program");
        println!("cargo:rustc-link-search=native={instdir}/sdk/lib");
        println!("cargo:rustc-link-search=native={instdir}/lib");
        // Core UNO runtime libs (available in stock LO and deb packages)
        println!("cargo:rustc-link-lib=uno_cppu");
        println!("cargo:rustc-link-lib=uno_sal");
        println!("cargo:rustc-link-lib=uno_salhelpergcc3");
        // Rust UNO C++ bridge — only available in LO 26.2+ built with --enable-rust-uno.
        // Optional: extensions work without it using the stub generated bindings.
        let bridge = std::path::Path::new(&instdir).join("program/librust_uno-cpplo.so");
        if bridge.exists() {
            println!("cargo:rustc-link-lib=rust_uno-cpplo");
        }
    }
    if let Ok(workdir) = std::env::var("WORKDIR") {
        println!("cargo:rustc-link-search=native={workdir}/LinkTarget/Library");
    }
}
BUILDEOF
''
