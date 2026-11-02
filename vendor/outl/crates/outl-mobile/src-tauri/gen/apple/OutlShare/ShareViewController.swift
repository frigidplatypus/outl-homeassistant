import OutlKit
import UIKit
import UniformTypeIdentifiers

/// Share extension (RFC 0363 step 3): text or a link shared to outl is
/// written to the capture inbox as one line and lands in today's journal
/// when the app next drains. No workspace access, no UI beyond a short
/// confirmation.
@objc(ShareViewController)
final class ShareViewController: UIViewController {
    private let label = UILabel()

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = UIColor.systemBackground.withAlphaComponent(0.95)
        label.textAlignment = .center
        label.font = .preferredFont(forTextStyle: .headline)
        label.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(label)
        NSLayoutConstraint.activate([
            label.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            label.centerYAnchor.constraint(equalTo: view.centerYAnchor),
        ])
        Task { await self.save() }
    }

    private func save() async {
        let (text, url) = await extract()
        let message: String
        if let line = SharedLine.compose(text: text, url: url), let inbox = CaptureInbox.shared() {
            let result = Result { try inbox.write(text: line, source: .share) }
            if case .success = result {
                CFNotificationCenterPostNotification(
                    CFNotificationCenterGetDarwinNotifyCenter(),
                    CFNotificationName(CaptureInbox.arrivedNotification as CFString),
                    nil, nil, true
                )
            }
            message = CaptureReply.message(for: result)
        } else {
            message = CaptureReply.empty
        }
        await MainActor.run { self.label.text = message }
        try? await Task.sleep(nanoseconds: 700_000_000)
        await MainActor.run {
            self.extensionContext?.completeRequest(returningItems: nil)
        }
    }

    private func extract() async -> (String?, URL?) {
        let providers = (extensionContext?.inputItems as? [NSExtensionItem] ?? [])
            .flatMap { $0.attachments ?? [] }
        var text: String?
        var url: URL?
        for provider in providers {
            if url == nil, provider.hasItemConformingToTypeIdentifier(UTType.url.identifier) {
                url = try? await provider.loadItem(forTypeIdentifier: UTType.url.identifier) as? URL
            }
            if text == nil, provider.hasItemConformingToTypeIdentifier(UTType.plainText.identifier) {
                text = try? await provider.loadItem(forTypeIdentifier: UTType.plainText.identifier) as? String
            }
        }
        return (text, url)
    }
}
