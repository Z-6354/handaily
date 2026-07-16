package com.handaily.hantransfer

import okhttp3.Call

class UploadSession {
    @Volatile
    var pauseRequested: Boolean = false

    @Volatile
    private var activeCall: Call? = null

    fun reset() {
        pauseRequested = false
        activeCall = null
    }

    fun requestPause() {
        pauseRequested = true
        activeCall?.cancel()
    }

    fun shouldStop(): Boolean = pauseRequested

    fun track(call: Call) {
        activeCall = call
    }

    fun clearCall() {
        activeCall = null
    }
}

class TransferPausedException(message: String = "上传已暂停") : Exception(message)
