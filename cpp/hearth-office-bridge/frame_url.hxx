// frame_url.hxx — Extract a document URL from a UNO XFrame.
//
// Walks XFrame → XController → XModel → getURL() and returns the URL as a
// UTF-8 std::string. Returns an empty string when no document is open or any
// step in the chain returns null.

#pragma once

#include <com/sun/star/frame/XFrame.hpp>
#include <com/sun/star/uno/Reference.hxx>
#include <string>

namespace hearth::office {

std::string get_document_url(
    const ::com::sun::star::uno::Reference< ::com::sun::star::frame::XFrame >& xFrame);

}  // namespace hearth::office
