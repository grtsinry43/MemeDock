import MemeDockBridge
import SwiftUI
import UIKit
import XCTest
@testable import MemeDock

@MainActor
final class AppShellTests: XCTestCase {
    func testNativeTabsAndRealPagesRenderAndRetainNavigation() async throws {
        let fixture = BridgeFixture()
        let session = LibrarySession(paths: fixture.paths)
        await session.start()
        let originalWindow = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
            .flatMap(\.windows).first { $0.isKeyWindow }
        var window: UIWindow?
        do {
            let library = try XCTUnwrap(session.library)
            let format = UIGraphicsImageRendererFormat()
            format.scale = 1
            let data = UIGraphicsImageRenderer(size: CGSize(width: 160, height: 160), format: format).pngData { _ in
                ("🥳" as NSString).draw(at: CGPoint(x: 8, y: 8), withAttributes: [.font: UIFont.systemFont(ofSize: 128)])
            }
            let imported = try await importTestMedia(library, bytes: data, name: "celebration.png", title: "Celebration")
            _ = try await library.createCollection(name: "Reactions", before: nil).awaitResult()
            _ = try await library.createTag(name: "Happy").awaitResult()
            await session.organize?.refresh()
            await session.refresh()
            let navigation = AppNavigation()
            let scene = try XCTUnwrap(originalWindow?.windowScene)
            let renderedWindow = UIWindow(windowScene: scene)
            window = renderedWindow
            let host = UIHostingController(rootView: AppTabsView(navigation: navigation, session: session)
                .environment(\.scenePhase, .active).environment(\.dynamicTypeSize, .large))
            renderedWindow.rootViewController = host
            renderedWindow.makeKeyAndVisible()
            for (tab, name) in [(AppTab.stickers, "Stickers"), (.organize, "Organize"), (.mine, "Mine")] {
                navigation.selected = tab
                try await Task.sleep(for: .milliseconds(300))
                host.view.layoutIfNeeded()
                let controller = try XCTUnwrap(findTabController(host), "Native tab controller is missing")
                XCTAssertEqual(controller.viewControllers?.count, 3)
                XCTAssertEqual(controller.selectedIndex, tab == .stickers ? 0 : tab == .organize ? 1 : 2)
                capture(renderedWindow, name: "App framework - " + name)
            }
            navigation.minePath = [.libraryInfo]
            try await Task.sleep(for: .milliseconds(300))
            capture(renderedWindow, name: "App framework - Library Information")
            navigation.selected = .stickers
            let detail = StickerDetailRoute(LibraryItem(sticker: imported.sticker, resource: nil))
            navigation.hero.present(detail, image: nil)
            try await Task.sleep(for: .milliseconds(300))
            navigation.selected = .organize
            navigation.selected = .stickers
            XCTAssertEqual(navigation.hero.presentation?.route, detail)
            XCTAssertEqual(navigation.minePath, [.libraryInfo])
            // Same hosted shell, dark appearance and accessibility-sized text.
            host.rootView = AppTabsView(navigation: navigation, session: session)
                .environment(\.scenePhase, .active).environment(\.dynamicTypeSize, .accessibility3)
            renderedWindow.overrideUserInterfaceStyle = .dark
            navigation.hero.reset()
            navigation.selected = .mine
            try await Task.sleep(for: .milliseconds(300))
            capture(renderedWindow, name: "App framework - Dark Library Information")
            renderedWindow.isHidden = true
            renderedWindow.rootViewController = nil
            originalWindow?.makeKeyAndVisible()
            await session.shutdown()
            try fixture.remove()
        } catch {
            window?.isHidden = true
            window?.rootViewController = nil
            originalWindow?.makeKeyAndVisible()
            await session.shutdown()
            try fixture.remove()
            throw error
        }
    }

    private func findTabController(_ controller: UIViewController) -> UITabBarController? {
        if let tabs = controller as? UITabBarController { return tabs }
        for child in controller.children {
            if let found = findTabController(child) { return found }
        }
        return nil
    }

    private func capture(_ window: UIWindow, name: String) {
        window.layoutIfNeeded()
        let image = UIGraphicsImageRenderer(bounds: window.bounds).image { _ in
            XCTAssertTrue(window.drawHierarchy(in: window.bounds, afterScreenUpdates: true))
        }
        let attachment = XCTAttachment(image: image)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
