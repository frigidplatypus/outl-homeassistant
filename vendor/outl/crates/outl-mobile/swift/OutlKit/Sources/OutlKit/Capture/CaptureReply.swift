import Foundation

/// What an entry point tells the user after writing to the inbox. The
/// line is saved on this phone, not yet in the journal or on other
/// devices, so the words say "saved", never "added" (RFC 0363).
public enum CaptureReply {
    public static let saved = "Saved, opens in outl."
    public static let empty = "Nothing to save."

    /// The reply for the outcome of a write.
    public static func message(for result: Result<URL, Error>) -> String {
        switch result {
        case .success:
            return saved
        case .failure(let error as CaptureInbox.WriteError) where error == .empty:
            return empty
        case .failure:
            return "Could not save. Open outl and try again."
        }
    }
}
