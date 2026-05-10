// comments_panel.hxx — XUIElementFactory hosting a Nextcloud comments panel.
//
// LibreOffice's sidebar framework calls createUIElement() with a resource URL
// like "private:resource/toolpanel/com.hearth.office/CommentsPanel" plus a
// PropertyValue sequence containing Frame/ParentWindow. We return an
// XUIElement whose getRealInterface returns an XToolPanel whose getWindow
// returns a multi-line read-only awt::Edit wrapping JSON-rendered comments
// fetched from the Rust ABI.

#pragma once

#include <com/sun/star/uno/Reference.hxx>
#include <com/sun/star/uno/Sequence.hxx>
#include <com/sun/star/uno/XComponentContext.hpp>
#include <rtl/ustring.hxx>

namespace hearth::office {

::com::sun::star::uno::Reference< ::com::sun::star::uno::XInterface >
    SAL_CALL CommentsPanel_createInstance(
        const ::com::sun::star::uno::Reference<
            ::com::sun::star::uno::XComponentContext >& xContext);

::rtl::OUString CommentsPanel_getImplementationName();

::com::sun::star::uno::Sequence< ::rtl::OUString >
    CommentsPanel_getSupportedServiceNames();

}  // namespace hearth::office
