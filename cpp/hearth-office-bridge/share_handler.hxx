// share_handler.hxx — ProtocolHandler for hearth: URLs (Share via Nextcloud)
//
// LibreOffice's dispatch framework routes URLs whose protocol matches the
// pattern registered in ProtocolHandler.xcu (`hearth:*`) to this service. The
// handler returns an XDispatch for paths it recognizes; the dispatch object
// forwards execution to Rust via hearth_share_via_nextcloud().

#pragma once

#include <com/sun/star/uno/Reference.hxx>
#include <com/sun/star/uno/Sequence.hxx>
#include <com/sun/star/uno/XComponentContext.hpp>
#include <rtl/ustring.hxx>

namespace hearth::office {

// Factory hooks consumed by cppu::ImplementationEntry in bridge.cxx.

::com::sun::star::uno::Reference< ::com::sun::star::uno::XInterface >
    SAL_CALL ShareHandler_createInstance(
        const ::com::sun::star::uno::Reference< ::com::sun::star::uno::XComponentContext >& xContext);

::rtl::OUString ShareHandler_getImplementationName();

::com::sun::star::uno::Sequence< ::rtl::OUString >
    ShareHandler_getSupportedServiceNames();

}  // namespace hearth::office
