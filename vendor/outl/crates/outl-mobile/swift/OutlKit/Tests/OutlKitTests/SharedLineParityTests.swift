import XCTest

@testable import OutlKit

/// `SharedLine.compose` answers every case in `shared-line-cases.json`.
///
/// That file is the contract, not this test. `outl_actions::capture::shared_line`
/// is the owner of the rule and reads the same cases from
/// `crates/outl-actions/tests/shared_line_parity.rs`; this Swift copy exists
/// only because an iOS share extension cannot load the Rust library
/// (RFC 0363).
///
/// Two implementations drift, and the direction this one drifts in is a share
/// that silently loses its link. So the cases live in one file and both suites
/// fail on a disagreement, instead of one of them being quietly right.
final class SharedLineParityTests: XCTestCase {
    private struct Fixture: Decodable {
        let cases: [Case]
    }

    private struct Case: Decodable {
        let why: String
        let text: String?
        let url: String?
        let line: String?
    }

    /// Walks up from this file to the repo root. `#filePath` is the only
    /// handle a `swift test` run has on a file outside its own package, and
    /// the fixture deliberately lives with the Rust owner rather than being
    /// copied in as a package resource — a copy is the drift this closes.
    private func fixtureURL() throws -> URL {
        var dir = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        for _ in 0..<12 {
            let candidate = dir.appendingPathComponent(
                "crates/outl-actions/tests/shared-line-cases.json")
            if FileManager.default.fileExists(atPath: candidate.path) {
                return candidate
            }
            dir = dir.deletingLastPathComponent()
        }
        throw XCTSkip("shared-line-cases.json not found above \(#filePath)")
    }

    func testComposeAnswersEverySharedCase() throws {
        let url = try fixtureURL()
        let fixture = try JSONDecoder().decode(Fixture.self, from: Data(contentsOf: url))

        XCTAssertGreaterThanOrEqual(
            fixture.cases.count, 10,
            "a shrunken fixture is a pin that stopped pinning")

        for c in fixture.cases {
            let got = SharedLine.compose(text: c.text, url: c.url.flatMap(URL.init(string:)))
            XCTAssertEqual(
                got, c.line,
                "case \"\(c.why)\": text=\(String(describing: c.text)) url=\(String(describing: c.url))"
            )
        }
    }
}
