// bridge.cxx — UNO component entry points for the hearth-office extension.
//
// LibreOffice loads libhearth_office_bridge.so and calls component_getFactory
// with each implementation name registered in hearth-office.components. The
// bridge returns an XSingleComponentFactory per service; when LO instantiates
// a component, the factory's createInstance hands back a UNO object whose
// methods forward into the Rust shared library (libhearth_office.so) via the
// extern "C" ABI declared in rust_ffi.hxx.
//
// Why a C++ shim and not pure Rust: upstream rust_uno (LO 26.2) ships
// interface-pointer wrappers but does not yet expose component-registration
// macros (`cppu_componentFactoryHelper` equivalent). When that lands upstream,
// this whole file can be retired in favor of pure-Rust component_getFactory.

#include <cppuhelper/factory.hxx>
#include <cppuhelper/implementationentry.hxx>
#include <com/sun/star/lang/XSingleComponentFactory.hpp>
#include <com/sun/star/uno/Reference.hxx>
#include <com/sun/star/uno/Sequence.hxx>
#include <rtl/ustring.hxx>
#include <sal/types.h>

#include "comments_panel.hxx"
#include "lock_status_controller.hxx"
#include "share_handler.hxx"

using namespace ::com::sun::star::uno;
using namespace ::com::sun::star::lang;

namespace {

// Service registration table — ImplementationEntry wires each
// (createInstance, getImplementationName, getSupportedServiceNames) triple
// into cppu::component_getFactoryHelper. The order is irrelevant — LO looks
// up by implementation name.
const ::cppu::ImplementationEntry kImplEntries[] = {
    {
        &::hearth::office::ShareHandler_createInstance,
        &::hearth::office::ShareHandler_getImplementationName,
        &::hearth::office::ShareHandler_getSupportedServiceNames,
        &::cppu::createSingleComponentFactory,
        nullptr, 0
    },
    {
        &::hearth::office::LockStatusController_createInstance,
        &::hearth::office::LockStatusController_getImplementationName,
        &::hearth::office::LockStatusController_getSupportedServiceNames,
        &::cppu::createSingleComponentFactory,
        nullptr, 0
    },
    {
        &::hearth::office::CommentsPanel_createInstance,
        &::hearth::office::CommentsPanel_getImplementationName,
        &::hearth::office::CommentsPanel_getSupportedServiceNames,
        &::cppu::createSingleComponentFactory,
        nullptr, 0
    },
    { nullptr, nullptr, nullptr, nullptr, nullptr, 0 }
};

}  // namespace

extern "C" {

SAL_DLLPUBLIC_EXPORT void* SAL_CALL
component_getFactory(const char* impl_name,
                     void* service_manager,
                     void* registry_key)
{
    return ::cppu::component_getFactoryHelper(
        impl_name, service_manager, registry_key, kImplEntries);
}

}  // extern "C"
