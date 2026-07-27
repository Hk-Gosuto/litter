package com.litter.android.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class AppLanguagePreferencesTest {
    @Test
    fun resolvesSupportedLanguageTags() {
        assertEquals(
            AppLanguage.ENGLISH,
            AppLanguagePreferences.languageForTag("en"),
        )
        assertEquals(
            AppLanguage.SIMPLIFIED_CHINESE,
            AppLanguagePreferences.languageForTag("zh-CN"),
        )
    }

    @Test
    fun languageTagsAreCaseInsensitiveAndUnknownTagsFallBack() {
        assertEquals(
            AppLanguage.SIMPLIFIED_CHINESE,
            AppLanguagePreferences.languageForTag("ZH-cn"),
        )
        assertNull(AppLanguagePreferences.languageForTag("fr"))
        assertNull(AppLanguagePreferences.languageForTag(null))
    }
}
