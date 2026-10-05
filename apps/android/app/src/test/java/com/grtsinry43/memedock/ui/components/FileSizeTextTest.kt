package com.grtsinry43.memedock.ui.components

import org.junit.Assert.assertEquals
import org.junit.Test
import java.util.Locale

class FileSizeTextTest {
    @Test fun smallImagesKeepTheirActualSizeAndLargerImagesUseReadableUnits() {
        assertEquals("202 B", formatFileSize(202, Locale.US))
        assertEquals("0 B", formatFileSize(0, Locale.US))
        assertEquals("1 KB", formatFileSize(1024, Locale.US))
        assertEquals("1.5 MB", formatFileSize(1572864, Locale.US))
        assertEquals("1 GB", formatFileSize(1073741824, Locale.US))
        assertEquals("1,5 MB", formatFileSize(1572864, Locale.GERMANY))
    }
}
