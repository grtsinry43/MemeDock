import XCTest
@testable import MemeDock

final class TapSequenceTests: XCTestCase {
    func testSevenTapsInsideTheWindowComplete() {
        var sequence = TapSequence()
        let start = Date(timeIntervalSince1970: 1_000)
        var fired = false
        for index in 0..<7 {
            fired = sequence.register(at: start.addingTimeInterval(Double(index) * 0.5))
        }
        XCTAssertTrue(fired)
    }

    func testTapsOutsideTheWindowDoNotComplete() {
        var sequence = TapSequence()
        let start = Date(timeIntervalSince1970: 1_000)
        var fired = false
        for index in 0..<7 {
            fired = sequence.register(at: start.addingTimeInterval(Double(index) * 0.6))
        }
        XCTAssertFalse(fired)
    }
}
