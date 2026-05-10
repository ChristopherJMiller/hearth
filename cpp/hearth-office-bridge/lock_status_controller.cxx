#include "lock_status_controller.hxx"
#include "frame_url.hxx"
#include "rust_ffi.hxx"

#include <cppuhelper/implbase2.hxx>
#include <cppuhelper/supportsservice.hxx>
#include <com/sun/star/awt/MouseEvent.hpp>
#include <com/sun/star/awt/Point.hpp>
#include <com/sun/star/awt/Rectangle.hpp>
#include <com/sun/star/awt/XGraphics.hpp>
#include <com/sun/star/awt/XWindow.hpp>
#include <com/sun/star/beans/PropertyValue.hpp>
#include <com/sun/star/frame/FeatureStateEvent.hpp>
#include <com/sun/star/frame/XFrame.hpp>
#include <com/sun/star/frame/XStatusbarController.hpp>
#include <com/sun/star/lang/EventObject.hpp>
#include <com/sun/star/lang/XEventListener.hpp>
#include <com/sun/star/lang/XInitialization.hpp>
#include <com/sun/star/lang/XServiceInfo.hpp>
#include <com/sun/star/ui/XStatusbarItem.hpp>

#include <array>
#include <mutex>
#include <string>

using namespace ::com::sun::star;
using ::rtl::OUString;

namespace hearth::office {

namespace {

constexpr char kImplName[] = "com.hearth.LockStatusController";
constexpr char kServiceName[] = "com.sun.star.frame.StatusbarController";

// XStatusbarController extends XComponent + XInitialization + XStatusListener
// + XUpdatable, so we get all of those by inheriting it. WeakImplHelper2 adds
// XServiceInfo (and XInterface/XTypeProvider via OWeakObject base).
class LockStatusController
    : public ::cppu::WeakImplHelper2<frame::XStatusbarController, lang::XServiceInfo>
{
public:
    explicit LockStatusController(uno::Reference<uno::XComponentContext> xContext)
        : mxContext(std::move(xContext)), mDisposed(false) {}

    // XInitialization — LO passes a 5-element PropertyValue sequence:
    //   Frame, CommandURL, StatusbarItem, ParentWindow, ModuleName
    void SAL_CALL initialize(const uno::Sequence<uno::Any>& aArguments) override
    {
        std::scoped_lock lock(mMutex);
        for (sal_Int32 i = 0; i < aArguments.getLength(); ++i) {
            beans::PropertyValue prop;
            if (!(aArguments[i] >>= prop)) continue;
            if (prop.Name == "Frame") {
                prop.Value >>= mxFrame;
            } else if (prop.Name == "StatusbarItem") {
                prop.Value >>= mxStatusbarItem;
            }
        }
    }

    // XUpdatable::update — refresh the lock status from Rust and push the
    // result into the status-bar item. LO calls this on focus/document-load
    // events; sufficient cadence for MVP. A polling timer can be added later
    // if staleness becomes a UX problem.
    void SAL_CALL update() override
    {
        uno::Reference<frame::XFrame> xFrame;
        uno::Reference<ui::XStatusbarItem> xItem;
        {
            std::scoped_lock lock(mMutex);
            if (mDisposed) return;
            xFrame = mxFrame;
            xItem = mxStatusbarItem;
        }
        if (!xItem.is()) return;

        const std::string url = get_document_url(xFrame);
        if (url.empty()) {
            xItem->setText(OUString());
            return;
        }

        std::array<uint8_t, 256> owner_buf{};
        const int32_t rc = hearth_check_lock_status(
            url.c_str(), owner_buf.data(), owner_buf.size());

        if (rc == 1) {
            // Locked. owner_buf holds a null-terminated UTF-8 owner name (may
            // be empty if the lock has no owner attribute).
            const char* owner =
                reinterpret_cast<const char*>(owner_buf.data());
            const OUString prefix("Locked");
            if (*owner != '\0') {
                xItem->setText(prefix + " by "
                    + OUString::createFromAscii(owner));
            } else {
                xItem->setText(prefix);
            }
        } else {
            // 0 unlocked, -1 not on Nextcloud / error → blank the indicator.
            xItem->setText(OUString());
        }
    }

    // XStatusListener — we don't subscribe to anything, but the interface is
    // mandatory because XStatusbarController inherits it. XStatusListener
    // also inherits XEventListener, which requires disposing().
    void SAL_CALL statusChanged(const frame::FeatureStateEvent& /*State*/) override {}
    void SAL_CALL disposing(const lang::EventObject& /*Source*/) override {}

    // XStatusbarController interaction methods — no custom UI for MVP.
    sal_Bool SAL_CALL mouseButtonDown(const awt::MouseEvent& /*ev*/) override { return false; }
    sal_Bool SAL_CALL mouseMove(const awt::MouseEvent& /*ev*/) override { return false; }
    sal_Bool SAL_CALL mouseButtonUp(const awt::MouseEvent& /*ev*/) override { return false; }
    void SAL_CALL command(const awt::Point& /*pos*/, sal_Int32 /*command*/,
                          sal_Bool /*mouseEvent*/, const uno::Any& /*data*/) override {}
    void SAL_CALL paint(const uno::Reference<awt::XGraphics>& /*xGraphics*/,
                        const awt::Rectangle& /*outputRect*/, sal_Int32 /*style*/) override {}
    void SAL_CALL click(const awt::Point& /*pos*/) override {}
    void SAL_CALL doubleClick(const awt::Point& /*pos*/) override {}

    // XComponent
    void SAL_CALL dispose() override
    {
        std::scoped_lock lock(mMutex);
        mDisposed = true;
        mxFrame.clear();
        mxStatusbarItem.clear();
    }
    void SAL_CALL addEventListener(
        const uno::Reference<lang::XEventListener>& /*xListener*/) override {}
    void SAL_CALL removeEventListener(
        const uno::Reference<lang::XEventListener>& /*xListener*/) override {}

    // XServiceInfo
    OUString SAL_CALL getImplementationName() override
    {
        return LockStatusController_getImplementationName();
    }
    sal_Bool SAL_CALL supportsService(const OUString& serviceName) override
    {
        return ::cppu::supportsService(this, serviceName);
    }
    uno::Sequence<OUString> SAL_CALL getSupportedServiceNames() override
    {
        return LockStatusController_getSupportedServiceNames();
    }

private:
    uno::Reference<uno::XComponentContext> mxContext;
    uno::Reference<frame::XFrame> mxFrame;
    uno::Reference<ui::XStatusbarItem> mxStatusbarItem;
    std::mutex mMutex;
    bool mDisposed;
};

}  // namespace

uno::Reference<uno::XInterface> SAL_CALL
LockStatusController_createInstance(const uno::Reference<uno::XComponentContext>& xContext)
{
    return static_cast<::cppu::OWeakObject*>(new LockStatusController(xContext));
}

OUString LockStatusController_getImplementationName()
{
    return OUString::createFromAscii(kImplName);
}

uno::Sequence<OUString> LockStatusController_getSupportedServiceNames()
{
    uno::Sequence<OUString> services(1);
    services.getArray()[0] = OUString::createFromAscii(kServiceName);
    return services;
}

}  // namespace hearth::office
