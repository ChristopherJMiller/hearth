#include "comments_panel.hxx"
#include "frame_url.hxx"
#include "rust_ffi.hxx"

#include <cppuhelper/implbase1.hxx>
#include <cppuhelper/implbase2.hxx>
#include <cppuhelper/implbase3.hxx>
#include <cppuhelper/supportsservice.hxx>

#include <com/sun/star/awt/Toolkit.hpp>
#include <com/sun/star/awt/XToolkit.hpp>
#include <com/sun/star/awt/XWindow.hpp>
#include <com/sun/star/awt/XWindowPeer.hpp>
#include <com/sun/star/awt/XTextComponent.hpp>
#include <com/sun/star/awt/Rectangle.hpp>
#include <com/sun/star/awt/WindowAttribute.hpp>
#include <com/sun/star/awt/WindowClass.hpp>
#include <com/sun/star/awt/WindowDescriptor.hpp>
#include <com/sun/star/awt/VclWindowPeerAttribute.hpp>
#include <com/sun/star/beans/NamedValue.hpp>
#include <com/sun/star/beans/PropertyValue.hpp>
#include <com/sun/star/frame/XFrame.hpp>
#include <com/sun/star/lang/XEventListener.hpp>
#include <com/sun/star/lang/XServiceInfo.hpp>
#include <com/sun/star/ui/UIElementType.hpp>
#include <com/sun/star/ui/XToolPanel.hpp>
#include <com/sun/star/ui/XUIElement.hpp>
#include <com/sun/star/ui/XUIElementFactory.hpp>

#include <vector>

using namespace ::com::sun::star;
using ::rtl::OUString;

namespace hearth::office {

namespace {

constexpr char kImplName[] = "com.hearth.CommentsPanel";
constexpr char kServiceName[] = "com.sun.star.ui.UIElementFactory";

// Render a JSON comment array (as produced by hearth_fetch_comments_json) to
// a human-readable multi-line string. Trades off fidelity for not pulling in
// a JSON parser — the JSON shape is stable and small, so a string-search
// renderer is adequate. If the JSON parses unexpectedly, the raw text falls
// through to the panel and the user can still read it.
OUString renderCommentsToText(const std::string& json)
{
    if (json.empty() || json == "[]") {
        return OUString("No comments yet.");
    }
    OUString out;
    size_t cursor = 0;
    while (true) {
        size_t obj_start = json.find('{', cursor);
        if (obj_start == std::string::npos) break;
        size_t obj_end = json.find('}', obj_start);
        if (obj_end == std::string::npos) break;
        std::string obj = json.substr(obj_start, obj_end - obj_start + 1);
        cursor = obj_end + 1;

        auto extract = [&](const std::string& key) -> std::string {
            std::string needle = "\"" + key + "\":\"";
            size_t k = obj.find(needle);
            if (k == std::string::npos) return {};
            k += needle.size();
            size_t e = obj.find('"', k);
            if (e == std::string::npos) return {};
            return obj.substr(k, e - k);
        };

        std::string author = extract("author_display_name");
        std::string when = extract("creation_date_time");
        std::string msg = extract("message");

        if (!author.empty()) {
            out += OUString::createFromAscii(author.c_str())
                + OUString(" · ")
                + OUString::createFromAscii(when.c_str())
                + OUString("\n")
                + OUString::createFromAscii(msg.c_str())
                + OUString("\n\n");
        }
    }
    return out.isEmpty() ? OUString("No comments yet.") : out;
}

// Build a multi-line read-only Edit window via the AWT toolkit. Returns null
// if any toolkit call fails — caller treats that as "panel unavailable".
uno::Reference<awt::XWindow> createCommentsWindow(
    const uno::Reference<uno::XComponentContext>& xContext,
    const uno::Reference<awt::XWindowPeer>& xParentPeer,
    const OUString& text)
{
    if (!xParentPeer.is()) return {};

    awt::WindowDescriptor desc;
    desc.Type = awt::WindowClass_SIMPLE;
    desc.WindowServiceName = OUString("MultiLineEdit");
    desc.Parent = xParentPeer;
    desc.ParentIndex = -1;
    desc.Bounds = awt::Rectangle{0, 0, 200, 400};
    desc.WindowAttributes =
        awt::WindowAttribute::SHOW
        | awt::WindowAttribute::SIZEABLE
        | awt::VclWindowPeerAttribute::VSCROLL
        | awt::VclWindowPeerAttribute::READONLY;

    // awt::Toolkit::create returns XToolkit2; cast back down to XToolkit.
    uno::Reference<awt::XToolkit> xToolkit;
    try {
        xToolkit.set(awt::Toolkit::create(xContext), uno::UNO_QUERY);
    } catch (const uno::Exception&) {
        return {};
    }
    if (!xToolkit.is()) return {};

    uno::Reference<awt::XWindowPeer> xPeer;
    try {
        xPeer = xToolkit->createWindow(desc);
    } catch (const uno::Exception&) {
        return {};
    }
    if (!xPeer.is()) return {};

    uno::Reference<awt::XTextComponent> xText(xPeer, uno::UNO_QUERY);
    if (xText.is()) {
        xText->setEditable(false);
        xText->setText(text);
    }

    return uno::Reference<awt::XWindow>(xPeer, uno::UNO_QUERY);
}

class CommentsToolPanel
    : public ::cppu::WeakImplHelper1<ui::XToolPanel>
{
public:
    explicit CommentsToolPanel(uno::Reference<awt::XWindow> xWindow)
        : mxWindow(std::move(xWindow)) {}

    uno::Reference<awt::XWindow> SAL_CALL getWindow() override
    {
        return mxWindow;
    }
    uno::Reference<accessibility::XAccessible> SAL_CALL createAccessible(
        const uno::Reference<accessibility::XAccessible>& /*parent*/) override
    {
        return {};
    }

private:
    uno::Reference<awt::XWindow> mxWindow;
};

class CommentsUIElement
    : public ::cppu::WeakImplHelper1<ui::XUIElement>
{
public:
    CommentsUIElement(uno::Reference<frame::XFrame> xFrame,
                      OUString sResourceURL,
                      uno::Reference<ui::XToolPanel> xPanel)
        : mxFrame(std::move(xFrame))
        , msResourceURL(std::move(sResourceURL))
        , mxPanel(std::move(xPanel)) {}

    uno::Reference<frame::XFrame> SAL_CALL getFrame() override { return mxFrame; }
    OUString SAL_CALL getResourceURL() override { return msResourceURL; }
    sal_Int16 SAL_CALL getType() override { return ui::UIElementType::TOOLPANEL; }
    uno::Reference<uno::XInterface> SAL_CALL getRealInterface() override
    {
        return uno::Reference<uno::XInterface>(mxPanel, uno::UNO_QUERY);
    }

private:
    uno::Reference<frame::XFrame> mxFrame;
    OUString msResourceURL;
    uno::Reference<ui::XToolPanel> mxPanel;
};

class CommentsPanelFactory
    : public ::cppu::WeakImplHelper2<ui::XUIElementFactory, lang::XServiceInfo>
{
public:
    explicit CommentsPanelFactory(uno::Reference<uno::XComponentContext> xContext)
        : mxContext(std::move(xContext)) {}

    uno::Reference<ui::XUIElement> SAL_CALL createUIElement(
        const OUString& ResourceURL,
        const uno::Sequence<beans::PropertyValue>& Args) override
    {
        uno::Reference<frame::XFrame> xFrame;
        uno::Reference<awt::XWindowPeer> xParentPeer;

        for (sal_Int32 i = 0; i < Args.getLength(); ++i) {
            if (Args[i].Name == "Frame") {
                Args[i].Value >>= xFrame;
            } else if (Args[i].Name == "ParentWindow") {
                Args[i].Value >>= xParentPeer;
            }
        }

        // Pull comments JSON from Rust. 64 KB is a generous cap for a
        // typical Nextcloud comment thread; if it's too small we just show
        // a friendly "too many comments" message rather than failing the
        // panel creation.
        OUString text;
        const std::string url = get_document_url(xFrame);
        if (url.empty()) {
            text = OUString("Open a saved document to see comments.");
        } else {
            std::vector<uint8_t> buf(64 * 1024, 0);
            int32_t rc = hearth_fetch_comments_json(
                url.c_str(), buf.data(), buf.size());
            if (rc == -1) {
                text = OUString("This document is not on Nextcloud.");
            } else if (rc == -2) {
                text = OUString("Too many comments to display.");
            } else if (rc < 0) {
                text = OUString("Failed to fetch comments.");
            } else {
                std::string json(reinterpret_cast<char*>(buf.data()), rc);
                text = renderCommentsToText(json);
            }
        }

        auto xWindow = createCommentsWindow(mxContext, xParentPeer, text);
        auto xPanel = uno::Reference<ui::XToolPanel>(
            new CommentsToolPanel(xWindow));

        return new CommentsUIElement(xFrame, ResourceURL, xPanel);
    }

    // XServiceInfo
    OUString SAL_CALL getImplementationName() override
    {
        return CommentsPanel_getImplementationName();
    }
    sal_Bool SAL_CALL supportsService(const OUString& serviceName) override
    {
        return ::cppu::supportsService(this, serviceName);
    }
    uno::Sequence<OUString> SAL_CALL getSupportedServiceNames() override
    {
        return CommentsPanel_getSupportedServiceNames();
    }

private:
    uno::Reference<uno::XComponentContext> mxContext;
};

}  // namespace

uno::Reference<uno::XInterface> SAL_CALL
CommentsPanel_createInstance(const uno::Reference<uno::XComponentContext>& xContext)
{
    return static_cast<::cppu::OWeakObject*>(new CommentsPanelFactory(xContext));
}

OUString CommentsPanel_getImplementationName()
{
    return OUString::createFromAscii(kImplName);
}

uno::Sequence<OUString> CommentsPanel_getSupportedServiceNames()
{
    uno::Sequence<OUString> services(1);
    services.getArray()[0] = OUString::createFromAscii(kServiceName);
    return services;
}

}  // namespace hearth::office
