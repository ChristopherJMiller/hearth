#include "frame_url.hxx"

#include <com/sun/star/frame/XController.hpp>
#include <com/sun/star/frame/XModel.hpp>
#include <rtl/ustring.hxx>
#include <rtl/string.hxx>
#include <rtl/textenc.h>

namespace hearth::office {

std::string get_document_url(
    const ::com::sun::star::uno::Reference< ::com::sun::star::frame::XFrame >& xFrame)
{
    if (!xFrame.is()) return {};

    auto xController = xFrame->getController();
    if (!xController.is()) return {};

    auto xModel = xController->getModel();
    if (!xModel.is()) return {};

    ::rtl::OUString url = xModel->getURL();
    if (url.isEmpty()) return {};

    ::rtl::OString utf8 = ::rtl::OUStringToOString(url, RTL_TEXTENCODING_UTF8);
    return std::string(utf8.getStr(), utf8.getLength());
}

}  // namespace hearth::office
