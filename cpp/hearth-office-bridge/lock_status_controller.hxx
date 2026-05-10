// lock_status_controller.hxx — XStatusbarController showing Nextcloud lock state.
//
// LO instantiates this once per status-bar slot bound to our command URL
// (configured in Addons.xcu). The controller polls Rust on update() and sets
// the StatusbarItem text accordingly. update() is called by LO when the
// status-bar redraws (focus changes, document load, manual refresh) — the MVP
// relies on that natural cadence rather than running a polling timer in C++.

#pragma once

#include <com/sun/star/uno/Reference.hxx>
#include <com/sun/star/uno/Sequence.hxx>
#include <com/sun/star/uno/XComponentContext.hpp>
#include <rtl/ustring.hxx>

namespace hearth::office {

::com::sun::star::uno::Reference< ::com::sun::star::uno::XInterface >
    SAL_CALL LockStatusController_createInstance(
        const ::com::sun::star::uno::Reference<
            ::com::sun::star::uno::XComponentContext >& xContext);

::rtl::OUString LockStatusController_getImplementationName();

::com::sun::star::uno::Sequence< ::rtl::OUString >
    LockStatusController_getSupportedServiceNames();

}  // namespace hearth::office
