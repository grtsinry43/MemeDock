import BackgroundTasks
import Foundation

@MainActor
enum BackgroundSyncScheduler {
    static let identifier = "com.grtsinry43.memedock.sync"
    static func register(session: LibrarySession) {
        BGTaskScheduler.shared.register(forTaskWithIdentifier: identifier, using: .main) { task in
            let operation = Task { @MainActor in
                await session.start()
                guard !Task.isCancelled, let sync = session.sync else {
                    task.setTaskCompleted(success: false)
                    return
                }
                let success = await sync.synchronize(automatically: true)
                sync.scheduleBackground()
                task.setTaskCompleted(success: success)
            }
            task.expirationHandler = {
                operation.cancel()
                Task { @MainActor in session.sync?.cancel() }
            }
        }
    }
    static func schedule(enabled: Bool) -> String? {
        BGTaskScheduler.shared.cancel(taskRequestWithIdentifier: identifier)
        guard enabled else { return nil }
        let request = BGProcessingTaskRequest(identifier: identifier)
        request.requiresNetworkConnectivity = true
        request.requiresExternalPower = false
        request.earliestBeginDate = Date(timeIntervalSinceNow: 15 * 60)
        do {
            try BGTaskScheduler.shared.submit(request)
            return nil
        } catch { return String(localized: "Background sync is currently unavailable. You can still sync manually.") }
    }
    static func cancel() { BGTaskScheduler.shared.cancel(taskRequestWithIdentifier: identifier) }
}
