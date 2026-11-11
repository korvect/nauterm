import Foundation
import Network

/// Forwards SecureCRT-style slash connect args to a running primary instance
/// before AppKit/`NSApplication` starts. Touching `NSApp` here would register
/// with WindowServer and flash a second Dock/app activation (e.g. from GNS3).
enum EphemeralConnectLauncher {
  /// Returns `true` and exits the process when the request was accepted.
  @discardableResult
  static func tryForwardAndExitIfNeeded() -> Bool {
    let argv = Array(CommandLine.arguments.dropFirst())
    guard let request = parseSlashConnectArgs(argv) else {
      return false
    }
    guard let endpoint = readEndpoint(),
          forward(request: request, endpoint: endpoint)
    else {
      return false
    }
    // Never create NSApplication on the secondary process.
    Darwin.exit(0)
  }

  private struct Endpoint {
    let port: UInt16
    let token: String
  }

  private struct ConnectRequest {
    let protocolName: String
    let host: String
    let port: Int
    let title: String?
    let username: String?
    let password: String?
    let identityPath: String?
  }

  private static func dataDirectoryURL() -> URL {
    if let override = ProcessInfo.processInfo.environment["NAUTERM_DATA_DIR"],
       !override.isEmpty
    {
      return URL(fileURLWithPath: override, isDirectory: true)
    }
    let home = FileManager.default.homeDirectoryForCurrentUser
    return home
      .appendingPathComponent("Library", isDirectory: true)
      .appendingPathComponent("Application Support", isDirectory: true)
      .appendingPathComponent("Nauterm", isDirectory: true)
  }

  private static func readEndpoint() -> Endpoint? {
    let fileURL = dataDirectoryURL().appendingPathComponent(
      "ephemeral-connect.endpoint"
    )
    guard let raw = try? String(contentsOf: fileURL, encoding: .utf8) else {
      return nil
    }
    let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
    if trimmed.isEmpty {
      return nil
    }
    if let data = trimmed.data(using: .utf8),
       let object = try? JSONSerialization.jsonObject(with: data),
       let map = object as? [String: Any]
    {
      let portValue: Int?
      if let port = map["port"] as? Int {
        portValue = port
      } else if let port = map["port"] as? String {
        portValue = Int(port.trimmingCharacters(in: .whitespacesAndNewlines))
      } else {
        portValue = nil
      }
      let token = (map["token"] as? String) ?? ""
      if let portValue, (1...65535).contains(portValue) {
        return Endpoint(port: UInt16(portValue), token: token)
      }
      return nil
    }

    // Legacy port-only endpoint files.
    let firstLine = trimmed.split(separator: "\n", maxSplits: 1).first.map(String.init)
    guard let firstLine,
          let port = Int(firstLine.trimmingCharacters(in: .whitespacesAndNewlines)),
          (1...65535).contains(port)
    else {
      return nil
    }
    return Endpoint(port: UInt16(port), token: "")
  }

  private static func forward(request: ConnectRequest, endpoint: Endpoint) -> Bool {
    var payload: [String: Any] = [
      "protocol": request.protocolName,
      "host": request.host,
      "port": request.port,
      "token": endpoint.token,
    ]
    if let title = request.title {
      payload["title"] = title
    }
    if let username = request.username {
      payload["username"] = username
    }
    if let password = request.password {
      payload["password"] = password
    }
    if let identityPath = request.identityPath {
      payload["identityPath"] = identityPath
    }
    guard JSONSerialization.isValidJSONObject(payload),
          let jsonData = try? JSONSerialization.data(withJSONObject: payload),
          var json = String(data: jsonData, encoding: .utf8)
    else {
      return false
    }
    json.append("\n")

    let semaphore = DispatchSemaphore(value: 0)
    var accepted = false
    let queue = DispatchQueue(label: "com.korvect.nauterm.ephemeral-forward")
    let connection = NWConnection(
      host: .ipv4(.loopback),
      port: NWEndpoint.Port(rawValue: endpoint.port)!,
      using: .tcp
    )
    connection.stateUpdateHandler = { state in
      switch state {
      case .ready:
        connection.send(
          content: json.data(using: .utf8),
          completion: .contentProcessed { error in
            if error != nil {
              connection.cancel()
              semaphore.signal()
              return
            }
            connection.receive(minimumIncompleteLength: 1, maximumLength: 256) {
              data,
              _,
              _,
              error in
              defer {
                connection.cancel()
                semaphore.signal()
              }
              guard error == nil, let data,
                    let line = String(data: data, encoding: .utf8)
              else {
                return
              }
              let response = line
                .split(separator: "\n", maxSplits: 1)
                .first
                .map(String.init)?
                .trimmingCharacters(in: .whitespacesAndNewlines)
              accepted = response == "ok"
            }
          }
        )
      case .failed, .cancelled:
        semaphore.signal()
      default:
        break
      }
    }
    connection.start(queue: queue)
    let waitResult = semaphore.wait(timeout: .now() + 1.0)
    if waitResult == .timedOut {
      connection.cancel()
    }
    return accepted
  }

  private static func parseSlashConnectArgs(_ args: [String]) -> ConnectRequest? {
    if args.isEmpty {
      return nil
    }

    var title: String?
    var protocolToken: String?
    var username: String?
    var password: String?
    var identityPath: String?
    var explicitPort: Int?
    var positional: [String] = []

    var index = 0
    while index < args.count {
      let raw = args[index].trimmingCharacters(in: .whitespacesAndNewlines)
      if raw.isEmpty {
        index += 1
        continue
      }
      let token = raw.uppercased()
      if token == "/T" {
        index += 1
        continue
      }
      if token == "/N" {
        guard index + 1 < args.count else { return nil }
        index += 1
        title = args[index].trimmingCharacters(in: .whitespacesAndNewlines)
        index += 1
        continue
      }
      if token == "/L" {
        guard index + 1 < args.count else { return nil }
        index += 1
        username = args[index].trimmingCharacters(in: .whitespacesAndNewlines)
        index += 1
        continue
      }
      if token == "/PASSWORD" {
        guard index + 1 < args.count else { return nil }
        index += 1
        password = args[index]
        index += 1
        continue
      }
      if token == "/IDENTITY" {
        guard index + 1 < args.count else { return nil }
        index += 1
        identityPath = args[index].trimmingCharacters(in: .whitespacesAndNewlines)
        index += 1
        continue
      }
      if token == "/P" {
        guard index + 1 < args.count else { return nil }
        index += 1
        explicitPort = Int(args[index].trimmingCharacters(in: .whitespacesAndNewlines))
        if explicitPort == nil {
          return nil
        }
        index += 1
        continue
      }
      if token == "/TELNET" || token == "/SSH2" || token == "/SSH" {
        protocolToken = token
        index += 1
        continue
      }
      if raw.hasPrefix("/") {
        index += 1
        continue
      }
      positional.append(raw)
      index += 1
    }

    guard let protocolToken else {
      return nil
    }

    if protocolToken == "/TELNET" {
      guard positional.count >= 2,
            let port = Int(positional[positional.count - 1]),
            isValidPort(port)
      else {
        return nil
      }
      let host = positional[positional.count - 2]
      return ConnectRequest(
        protocolName: "telnet",
        host: host,
        port: port,
        title: emptyToNil(title),
        username: nil,
        password: nil,
        identityPath: nil
      )
    }

    guard let host = positional.last else {
      return nil
    }
    let port = explicitPort ?? 22
    guard isValidPort(port), let user = emptyToNil(username) else {
      return nil
    }
    return ConnectRequest(
      protocolName: "ssh",
      host: host,
      port: port,
      title: emptyToNil(title),
      username: user,
      password: password,
      identityPath: emptyToNil(identityPath)
    )
  }

  private static func isValidPort(_ port: Int) -> Bool {
    (1...65535).contains(port)
  }

  private static func emptyToNil(_ value: String?) -> String? {
    guard let trimmed = value?.trimmingCharacters(in: .whitespacesAndNewlines),
          !trimmed.isEmpty
    else {
      return nil
    }
    return trimmed
  }
}
