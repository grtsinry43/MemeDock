// swift-tools-version: 5.10
import PackageDescription

// Swift sources only. The Rust sources stay with Cargo, and the UniFFI
// bindings under Generated/ are produced by tools/macos/build.sh, which also
// passes the linker search path of libmemedock_ffi.dylib:
//
//   swift build -Xlinker -L<dir> -Xlinker -rpath -Xlinker <dir>
let package = Package(
    name: "MemeDock",
    platforms: [.macOS(.v14)],
    products: [
        .executable(name: "MemeDock", targets: ["MemeDock"])
    ],
    targets: [
        .systemLibrary(name: "MemeDockBridgeFFI", path: "Generated/FFI"),
        .target(
            name: "MemeDockBridge",
            dependencies: ["MemeDockBridgeFFI"],
            path: "Generated/Bridge"
        ),
        .executableTarget(
            name: "MemeDock",
            dependencies: ["MemeDockBridge"],
            path: "Sources/MemeDock",
            linkerSettings: [
                .linkedLibrary("memedock_ffi")
            ]
        ),
        // Tests link against the executable target; they cover the platform
        // rules around the drag hand-off and Quick Look, not the core, which
        // has its own Rust tests.
        .testTarget(
            name: "MemeDockTests",
            dependencies: ["MemeDock"],
            path: "Tests/MemeDockTests"
        ),
    ]
)
