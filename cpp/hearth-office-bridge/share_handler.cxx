// share_handler.cxx — XDispatchProvider that maps hearth: URLs to Rust calls.

#include "share_handler.hxx"
#include "frame_url.hxx"
#include "rust_ffi.hxx"

// SDK ships only the legacy numbered WeakImplHelperN variants — the variadic
// implbase.hxx is internal-only.
#include <cppuhelper/implbase1.hxx>
#include <cppuhelper/implbase3.hxx>
#include <cppuhelper/supportsservice.hxx>
#include <com/sun/star/frame/XDispatch.hpp>
#include <com/sun/star/frame/XDispatchProvider.hpp>
#include <com/sun/star/frame/XFrame.hpp>
#include <com/sun/star/frame/XStatusListener.hpp>
#include <com/sun/star/lang/XInitialization.hpp>
#include <com/sun/star/lang/XServiceInfo.hpp>
#include <com/sun/star/util/URL.hpp>

#include <mutex>
#include <string>

using namespace ::com::sun::star;
using ::rtl::OUString;

namespace hearth::office {

namespace {

constexpr char kImplName[] = "com.hearth.ShareHandler";
constexpr char kServiceName[] = "com.sun.star.frame.ProtocolHandler";

// XDispatch implementation: when LO calls dispatch(), forward into Rust with
// the active XFrame. The frame pointer Rust receives is an opaque
// css::frame::XFrame* — Rust does not dereference it, only passes it back when
// fetching the document URL via the (currently stubbed) rust_uno bindings.
class ShareDispatch
    : public ::cppu::WeakImplHelper1<frame::XDispatch>
{
public:
    explicit ShareDispatch(uno::Reference<frame::XFrame> xFrame)
        : mxFrame(std::move(xFrame)) {}

    void SAL_CALL dispatch(const util::URL& aURL,
                           const uno::Sequence<beans::PropertyValue>& /*lArgs*/) override
    {
        if (!aURL.Path.equalsAscii("ShareViaNextcloud")) {
            return;
        }
        // Resolve the document URL on the C++ side (we have real UNO bindings
        // here; the Rust crate intentionally avoids UNO interop). Pass the
        // resulting UTF-8 URL into Rust, which handles the Nextcloud OCS call.
        const std::string url = get_document_url(mxFrame);
        if (url.empty()) {
            // No document open or unsaved buffer — nothing to share.
            return;
        }
        (void) hearth_share_via_nextcloud(url.c_str());
    }

    void SAL_CALL addStatusListener(
        const uno::Reference<frame::XStatusListener>& /*xListener*/,
        const util::URL& /*aURL*/) override
    {
        // No status to report — the share action is a one-shot, not a state.
    }

    void SAL_CALL removeStatusListener(
        const uno::Reference<frame::XStatusListener>& /*xListener*/,
        const util::URL& /*aURL*/) override {}

private:
    uno::Reference<frame::XFrame> mxFrame;
};

class ShareHandler
    : public ::cppu::WeakImplHelper3<
          frame::XDispatchProvider,
          lang::XInitialization,
          lang::XServiceInfo>
{
public:
    explicit ShareHandler(uno::Reference<uno::XComponentContext> xContext)
        : mxContext(std::move(xContext)) {}

    // XInitialization — LO passes the parent XFrame as arg[0] when the
    // ProtocolHandler is created.
    void SAL_CALL initialize(const uno::Sequence<uno::Any>& aArguments) override
    {
        std::scoped_lock lock(mMutex);
        if (aArguments.hasElements()) {
            aArguments[0] >>= mxFrame;
        }
    }

    // XDispatchProvider
    uno::Reference<frame::XDispatch> SAL_CALL queryDispatch(
        const util::URL& aURL,
        const OUString& /*sTargetFrameName*/,
        sal_Int32 /*nSearchFlags*/) override
    {
        if (!aURL.Protocol.equalsAscii("hearth:")) {
            return {};
        }
        if (aURL.Path.equalsAscii("ShareViaNextcloud")) {
            std::scoped_lock lock(mMutex);
            return new ShareDispatch(mxFrame);
        }
        // Other hearth: paths (e.g. ShowComments) handled by their own
        // services — return null so LO continues searching.
        return {};
    }

    uno::Sequence<uno::Reference<frame::XDispatch>> SAL_CALL queryDispatches(
        const uno::Sequence<frame::DispatchDescriptor>& seqDescriptors) override
    {
        const sal_Int32 n = seqDescriptors.getLength();
        uno::Sequence<uno::Reference<frame::XDispatch>> out(n);
        auto* outArr = out.getArray();
        for (sal_Int32 i = 0; i < n; ++i) {
            outArr[i] = queryDispatch(seqDescriptors[i].FeatureURL,
                                      seqDescriptors[i].FrameName,
                                      seqDescriptors[i].SearchFlags);
        }
        return out;
    }

    // XServiceInfo
    OUString SAL_CALL getImplementationName() override
    {
        return ShareHandler_getImplementationName();
    }
    sal_Bool SAL_CALL supportsService(const OUString& serviceName) override
    {
        return ::cppu::supportsService(this, serviceName);
    }
    uno::Sequence<OUString> SAL_CALL getSupportedServiceNames() override
    {
        return ShareHandler_getSupportedServiceNames();
    }

private:
    uno::Reference<uno::XComponentContext> mxContext;
    uno::Reference<frame::XFrame> mxFrame;
    std::mutex mMutex;
};

}  // namespace

uno::Reference<uno::XInterface> SAL_CALL
ShareHandler_createInstance(const uno::Reference<uno::XComponentContext>& xContext)
{
    return static_cast<::cppu::OWeakObject*>(new ShareHandler(xContext));
}

OUString ShareHandler_getImplementationName()
{
    return OUString::createFromAscii(kImplName);
}

uno::Sequence<OUString> ShareHandler_getSupportedServiceNames()
{
    uno::Sequence<OUString> services(1);
    services.getArray()[0] = OUString::createFromAscii(kServiceName);
    return services;
}

}  // namespace hearth::office
