import MemeDockBridge
import XCTest
@testable import MemeDock

@MainActor
final class OrganizeTests: XCTestCase {
    func testRealCatalogAndIsolatedCollectionAndTagQueries() async throws {
        let fixture = BridgeFixture()
        let session = LibrarySession(paths: fixture.paths)
        await session.start()
        do {
            let library = try XCTUnwrap(session.library)
            let home = try XCTUnwrap(session.browser)
            let model = try XCTUnwrap(session.organize)
            let first = try await importTestMedia(library, bytes: testPNG(variant: 71), name: "first.png", title: "First")
            _ = try await importTestPNG(library, variant: 72, title: "Second")
            let collection = try await library.createCollection(name: "Reactions", before: nil).awaitResult()
            let tag = try await library.createTag(name: "Happy").awaitResult()
            _ = try await library.setStickerOrganization(
                target: EntityReference(id: first.sticker.id, generation: first.sticker.lifecycle.generation),
                changeCollection: true,
                collection: EntityReference(id: collection.id, generation: collection.lifecycle.generation),
                tags: [EntityReference(id: tag.id, generation: tag.lifecycle.generation)]
            ).awaitResult()
            await model.refresh()
            // The live event feed may replace an explicit refresh with a newer query.
            for _ in 0..<100 where model.collections.first?.count != 1 || model.tags.isEmpty {
                await model.waitUntilSettled()
                try await Task.sleep(for: .milliseconds(10))
            }
            XCTAssertEqual(model.collections.first?.count, 1)
            XCTAssertEqual(model.tags.map(\.name), ["Happy"])
            let collectionRoute = GroupRoute(filter: .collection(collection.id), generation: collection.lifecycle.generation)
            let tagRoute = GroupRoute(filter: .tag(tag.id), generation: tag.lifecycle.generation)
            let grouped = try XCTUnwrap(session.makeGroupBrowser(for: collectionRoute))
            let tagged = try XCTUnwrap(session.makeGroupBrowser(for: tagRoute))
            home.filter = .all
            home.searchText = "Second"
            await home.refresh()
            await grouped.refresh()
            await tagged.refresh()
            XCTAssertEqual(grouped.items.map(\.id), [first.sticker.id])
            XCTAssertEqual(tagged.items.map(\.id), [first.sticker.id])
            XCTAssertEqual(home.items.map(\.sticker.title), ["Second"])
            XCTAssertEqual(home.searchText, "Second")
            home.visibleStickerID = home.items.first?.id
            let visible = home.visibleStickerID
            await session.refreshStatistics()
            XCTAssertEqual(home.searchText, "Second")
            XCTAssertEqual(home.visibleStickerID, visible)
            XCTAssertNotNil(session.statistics)
            let renamed = try await library.renameTag(
                target: EntityReference(id: tag.id, generation: tag.lifecycle.generation), name: "Joy"
            ).awaitResult()
            for _ in 0..<100 where model.tags.first?.name != "Joy" {
                try await Task.sleep(for: .milliseconds(10))
            }
            XCTAssertEqual(model.name(for: tagRoute), renamed.name)
            _ = try await library.deleteTag(target: EntityReference(id: tag.id, generation: tag.lifecycle.generation)).awaitResult()
            for _ in 0..<100 where model.name(for: tagRoute) != nil {
                try await Task.sleep(for: .milliseconds(10))
            }
            XCTAssertNil(model.name(for: tagRoute))
            XCTAssertNil(session.makeGroupBrowser(for: tagRoute))
            await session.shutdown()
            XCTAssertTrue(grouped.items.isEmpty)
            XCTAssertTrue(tagged.items.isEmpty)
            XCTAssertTrue(model.collections.isEmpty)
            XCTAssertNil(session.organize)
            try fixture.remove()
        } catch {
            await session.shutdown()
            try fixture.remove()
            throw error
        }
    }

    func testClosingDuringCatalogReadCannotPublishLateResults() async throws {
        try await withTestLibrary { library, _ in
            _ = try await library.createTag(name: "Existing").awaitResult()
            let model = OrganizeModel(library: library)
            model.scheduleRefresh()
            await model.close()
            await model.waitUntilSettled()
            XCTAssertTrue(model.tags.isEmpty)
            XCTAssertFalse(model.isLoading)
            model.scheduleRefresh()
            XCTAssertFalse(model.isLoading)
        }
    }
}
