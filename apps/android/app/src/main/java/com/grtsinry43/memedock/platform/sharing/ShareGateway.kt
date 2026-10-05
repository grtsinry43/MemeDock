package com.grtsinry43.memedock.platform.sharing

import android.app.Activity
import com.grtsinry43.memedock.data.library.ShareArtifact

interface ShareGateway { fun share(activity: Activity, artifact: ShareArtifact) }
