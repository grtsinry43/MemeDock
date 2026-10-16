vcpkg_check_linkage(ONLY_STATIC_LIBRARY)
vcpkg_download_distfile(ARCHIVE
    URLS "https://codeload.github.com/msrd0/rlottie-telegram/tar.gz/f435c0a4f364ce39f7e4eee9ddc7118573da9bf9"
    FILENAME "rlottie-telegram-f435c0a4.tar.gz"
    SHA512 46b8113703ae0e67534baa6a8ac5f317c632fd96e88ae0a5e3eb7140577ef92becdfe93d096505a973758173bacc78e75b8827803e306aaf67816bda366b64cb
)
vcpkg_extract_source_archive(SOURCE_PATH ARCHIVE "${ARCHIVE}" PATCHES msvc-build.patch)
vcpkg_cmake_configure(
    SOURCE_PATH "${SOURCE_PATH}"
    OPTIONS
        -DCMAKE_POLICY_VERSION_MINIMUM=3.5
        -DLOTTIE_MODULE=OFF
        -DLOTTIE_THREAD=OFF
        -DLOTTIE_CACHE=OFF
        -DLIB_INSTALL_DIR=lib
)
vcpkg_cmake_install()
vcpkg_cmake_config_fixup(PACKAGE_NAME rlottie CONFIG_PATH lib/cmake/rlottie)
vcpkg_fixup_pkgconfig()
vcpkg_copy_pdbs()
vcpkg_install_copyright(FILE_LIST "${SOURCE_PATH}/COPYING")
