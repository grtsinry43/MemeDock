package com.grtsinry43.memedock.platform.clipboard

import com.grtsinry43.memedock.data.library.ShareArtifact

interface ClipboardGateway {
    fun copy(artifact: ShareArtifact, reference: String)
    /** Caller must have a focused foreground window. An unavailable read throws. */
    fun observedReference(): String?
}
