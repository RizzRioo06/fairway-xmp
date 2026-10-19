// Experimental, fixed-capability daemon. The identical signed binary is its client.
import Foundation
import Darwin

@_silgen_name("fairway_hosts_apply")
func applyHosts(_ bytes: UnsafePointer<UInt8>, _ count: Int) -> UnsafeMutablePointer<CChar>?
@_silgen_name("fairway_hosts_free_error")
func freeError(_ error: UnsafeMutablePointer<CChar>)

@objc protocol FairwayHostsProtocol {
    func synchronize(_ request: Data, reply: @escaping (String?) -> Void)
}

final class HostsService: NSObject, FairwayHostsProtocol {
    private let writes = DispatchQueue(label: "Fairway hosts writes")
    private let pending = DispatchSemaphore(value: 4)
    func synchronize(_ request: Data, reply: @escaping (String?) -> Void) {
        guard !request.isEmpty, request.count <= 65536 else {
            reply("Invalid address request size."); return
        }
        guard pending.wait(timeout: .now()) == .success else {
            reply("Address helper is busy. Try again."); return
        }
        writes.async {
            defer { self.pending.signal() }
            let error = request.withUnsafeBytes { bytes in
                applyHosts(bytes.bindMemory(to: UInt8.self).baseAddress!, request.count)
            }
            if let error = error {
                let message = String(cString: error)
                freeError(error)
                reply(message)
            } else { reply(nil) }
        }
    }
}

final class ListenerDelegate: NSObject, NSXPCListenerDelegate {
    let service = HostsService()
    func listener(_ listener: NSXPCListener, shouldAcceptNewConnection connection: NSXPCConnection) -> Bool {
        // launchd's listener enforces the signing requirement before this callback.
        connection.exportedInterface = NSXPCInterface(with: FairwayHostsProtocol.self)
        connection.exportedObject = service
        connection.resume()
        return true
    }
}

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8)); exit(1)
}

switch CommandLine.arguments.dropFirst().first {
case "--server":
    guard geteuid() == 0 else { fail("Use the approved launchd service.") }
    let delegate = ListenerDelegate()
    let listener = NSXPCListener(machServiceName: helperLabel)
    listener.setConnectionCodeSigningRequirement(helperRequirement)
    listener.delegate = delegate
    listener.resume()
    withExtendedLifetime(delegate) { dispatchMain() }
case "--client":
    guard geteuid() != 0 else { fail("Run the address client as the signed-in user.") }
    var request = Data()
    while request.count <= 65536 {
        let chunk = FileHandle.standardInput.readData(ofLength: min(8192, 65537 - request.count))
        if chunk.isEmpty { break }
        request.append(chunk)
    }
    guard !request.isEmpty, request.count <= 65536 else { fail("Invalid address request size.") }
    let connection = NSXPCConnection(machServiceName: helperLabel, options: .privileged)
    connection.setCodeSigningRequirement(helperRequirement)
    connection.remoteObjectInterface = NSXPCInterface(with: FairwayHostsProtocol.self)
    connection.invalidationHandler = { fail("Address helper disconnected. Check its approval in System Settings.") }
    connection.interruptionHandler = { fail("Address helper was interrupted. Try again.") }
    connection.resume()
    guard let proxy = connection.remoteObjectProxyWithErrorHandler({ error in fail(error.localizedDescription) }) as? FairwayHostsProtocol else {
        fail("Cannot connect to the signed address helper.")
    }
    proxy.synchronize(request) { error in
        if let error = error { fail(error) }
        exit(0)
    }
    DispatchQueue.global().asyncAfter(deadline: .now() + 30) { fail("Address helper timed out. Check System Settings approval and retry.") }
    dispatchMain()
default:
    fail("Expected --client or --server.")
}
