import AppIntents
import Foundation
import OutlKit

/// "Add to today" (RFC 0363 step 2): the App Intent behind Shortcuts,
/// Siri and the Action button. It only writes the line to the capture
/// inbox and posts the arrival notification; the app turns it into a
/// block on its next drain. It does not open the app.
///
/// `AppIntents` is weak-linked (`-weak_framework AppIntents` in the app
/// target) because the app still runs on iOS 15.
@available(iOS 16.0, *)
struct AddToTodayIntent: AppIntent {
    static var title: LocalizedStringResource = "Add to today"
    static var description = IntentDescription("Save a line for today's journal in outl.")
    static var openAppWhenRun = false

    @Parameter(title: "Text", requestValueDialog: "What should go in today's journal?")
    var text: String

    static var parameterSummary: some ParameterSummary {
        Summary("Add \(\.$text) to today")
    }

    func perform() async throws -> some IntentResult & ProvidesDialog {
        guard let inbox = CaptureInbox.shared() else {
            return .result(dialog: "Could not save. Open outl and try again.")
        }
        let result = Result { try inbox.write(text: text, source: .intent) }
        if case .success = result {
            CFNotificationCenterPostNotification(
                CFNotificationCenterGetDarwinNotifyCenter(),
                CFNotificationName(CaptureInbox.arrivedNotification as CFString),
                nil, nil, true
            )
        }
        return .result(dialog: IntentDialog(stringLiteral: CaptureReply.message(for: result)))
    }
}

@available(iOS 16.0, *)
struct OutlShortcuts: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: AddToTodayIntent(),
            phrases: [
                "Add to today in \(.applicationName)",
                "Add a line to \(.applicationName)",
            ],
            shortTitle: "Add to today",
            systemImageName: "plus.circle"
        )
    }
}
