import Foundation

public final class CurseClient {
    public init() {}

    public func getmodslist(query: String) -> String? {
        return withCString(query) { cQuery in
            guard let raw = cc_getmodslistjson(cQuery) else { return nil }
            let result = String(cString: raw)
            cc_free_string(raw)
            return result
        }
    }

    public func getmodfiles(dllink: String) -> String? {
        return withCString(dllink) { cLink in
            guard let raw = cc_getmodfilesjson(cLink) else { return nil }
            let result = String(cString: raw)
            cc_free_string(raw)
            return result
        }
    }

    private func withCString<T>(_ value: String, _ body: (UnsafePointer<CChar>) -> T) -> T {
        return value.withCString { body($0) }
    }
}
