import Foundation

/// Turn what the share sheet hands over into the one line that goes to
/// the inbox: the text, the URL, or both as `text url` when the URL is
/// not already in the text. Newlines collapse to spaces so a share is
/// always one block.
public enum SharedLine {
    public static func compose(text: String?, url: URL?) -> String? {
        let body = (text ?? "")
            .components(separatedBy: .newlines)
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
            .joined(separator: " ")
        let link = url?.absoluteString ?? ""
        switch (body.isEmpty, link.isEmpty) {
        case (true, true): return nil
        case (true, false): return link
        case (false, true): return body
        case (false, false): return body.contains(link) ? body : "\(body) \(link)"
        }
    }
}
