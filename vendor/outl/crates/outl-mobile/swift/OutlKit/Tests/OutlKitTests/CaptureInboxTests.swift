import XCTest

@testable import OutlKit

final class CaptureInboxTests: XCTestCase {
    private var dir: URL!

    override func setUpWithError() throws {
        dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("outl-inbox-\(UUID().uuidString)", isDirectory: true)
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: dir)
    }

    func testAWriteLeavesOneCompleteFileNamedByItsULID() throws {
        let inbox = CaptureInbox(directory: dir)
        let url = try inbox.write(text: "  call the venue \n", source: .intent, now: Date(timeIntervalSince1970: 1_760_000_000))

        let names = try FileManager.default.contentsOfDirectory(atPath: dir.path)
        XCTAssertEqual(names, [url.lastPathComponent], "no temp file is left behind")
        XCTAssertTrue(url.lastPathComponent.hasSuffix(".json"))
        XCTAssertEqual(url.deletingPathExtension().lastPathComponent.count, 26)

        let record = try JSONDecoder().decode(CaptureRecord.self, from: Data(contentsOf: url))
        XCTAssertEqual(record, CaptureRecord(text: "call the venue", capturedAtMs: 1_760_000_000_000, source: .intent))
    }

    func testTheFileUsesTheKeysTheRustReaderExpects() throws {
        let url = try CaptureInbox(directory: dir).write(text: "x", source: .share)
        let json = try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any]
        XCTAssertEqual(Set(json?.keys ?? [:].keys), ["text", "captured_at_ms", "source"])
        XCTAssertEqual(json?["source"] as? String, "share")
    }

    func testWhitespaceIsRefused() {
        XCTAssertThrowsError(try CaptureInbox(directory: dir).write(text: " \n ", source: .widget)) { error in
            XCTAssertEqual(error as? CaptureInbox.WriteError, .empty)
        }
    }

    func testNamesSortInCaptureOrder() throws {
        let inbox = CaptureInbox(directory: dir)
        let base = Date(timeIntervalSince1970: 1_760_000_000)
        let later = try inbox.write(text: "b", source: .intent, now: base.addingTimeInterval(1), random: { 0 })
        let earlier = try inbox.write(text: "a", source: .intent, now: base, random: { UInt64.max })
        XCTAssertLessThan(earlier.lastPathComponent, later.lastPathComponent)
    }

    func testULIDEncodesTimeInTheFirstTenCharacters() {
        XCTAssertEqual(ULID.make(ms: 0, random: { 0 }), String(repeating: "0", count: 26))
        XCTAssertEqual(ULID.make(ms: 1, random: { 0 }).prefix(10), "0000000001")
        let all = ULID.make(ms: 0xFFFF_FFFF_FFFF, random: { UInt64.max })
        XCTAssertEqual(all, "7ZZZZZZZZZZZZZZZZZZZZZZZZZ")
    }
}

final class CaptureReplyTests: XCTestCase {
    func testTheReplyNeverClaimsTheLineIsAlreadyInTheJournal() {
        XCTAssertEqual(CaptureReply.message(for: .success(URL(fileURLWithPath: "/x"))), "Saved, opens in outl.")
        XCTAssertEqual(CaptureReply.message(for: .failure(CaptureInbox.WriteError.empty)), "Nothing to save.")
        XCTAssertFalse(CaptureReply.saved.lowercased().contains("added"))
    }
}

// `SharedLine.compose` is covered by `SharedLineParityTests`, which reads
// `crates/outl-actions/tests/shared-line-cases.json` — the same cases the
// Rust owner answers. The hand-written copy that used to live here
// asserted four of them inline, so a case added to the fixture was not
// added to it: two Swift spellings of one rule, only one of them governed
// by the contract. That is the drift the fixture exists to remove.
