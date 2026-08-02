package com.litter.android.state

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.codex_mobile_client.AuthMode

class VoiceTranscriptionRoutingTest {
    @Test
    fun codexAuthModesOnlyUseCompatibleTranscriptionEndpoints() {
        assertEquals(VoiceTranscriptionEndpoint.OPENAI, voiceTranscriptionEndpoint(AuthMode.API_KEY))
        assertEquals(VoiceTranscriptionEndpoint.CHATGPT, voiceTranscriptionEndpoint(AuthMode.CHATGPT))
        assertEquals(
            VoiceTranscriptionEndpoint.CHATGPT,
            voiceTranscriptionEndpoint(AuthMode.CHATGPT_AUTH_TOKENS),
        )
        assertEquals(
            VoiceTranscriptionEndpoint.CHATGPT,
            voiceTranscriptionEndpoint(AuthMode.PERSONAL_ACCESS_TOKEN),
        )

        assertNull(voiceTranscriptionEndpoint(AuthMode.HEADERS))
        assertNull(voiceTranscriptionEndpoint(AuthMode.AGENT_IDENTITY))
        assertNull(voiceTranscriptionEndpoint(AuthMode.BEDROCK_API_KEY))
        assertNull(voiceTranscriptionEndpoint(null))
    }
}
