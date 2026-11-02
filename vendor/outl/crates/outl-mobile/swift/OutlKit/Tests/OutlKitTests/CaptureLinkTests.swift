import XCTest

@testable import OutlKit

final class CaptureLinkTests: XCTestCase {
    func testTheWidgetOpensTheCaptureSheetLink() {
        XCTAssertEqual(CaptureLink.capture.absoluteString, "outl://capture")
        XCTAssertEqual(CaptureLink.capture.scheme, "outl")
        XCTAssertEqual(CaptureLink.capture.host, "capture")
    }
}
