package com.litter.android.ui

import android.content.Context
import android.content.res.Configuration
import java.util.Locale

enum class AppLanguage(val languageTag: String, val nativeDisplayName: String) {
    ENGLISH("en", "English"),
    SIMPLIFIED_CHINESE("zh-CN", "简体中文"),
}

object AppLanguagePreferences {
    private const val PREFERENCES_NAME = "litter_ui"
    private const val LANGUAGE_KEY = "app_language"

    fun current(context: Context): AppLanguage {
        val storedTag = context
            .getSharedPreferences(PREFERENCES_NAME, Context.MODE_PRIVATE)
            .getString(LANGUAGE_KEY, null)
        return languageForTag(storedTag) ?: inferredSystemLanguage()
    }

    fun set(context: Context, language: AppLanguage) {
        context.applicationContext
            .getSharedPreferences(PREFERENCES_NAME, Context.MODE_PRIVATE)
            .edit()
            .putString(LANGUAGE_KEY, language.languageTag)
            .apply()
    }

    fun wrap(context: Context): Context {
        val language = current(context)
        val locale = Locale.forLanguageTag(language.languageTag)
        Locale.setDefault(locale)
        val configuration = Configuration(context.resources.configuration)
        configuration.setLocale(locale)
        configuration.setLayoutDirection(locale)
        return context.createConfigurationContext(configuration)
    }

    internal fun languageForTag(languageTag: String?): AppLanguage? =
        AppLanguage.entries.firstOrNull {
            it.languageTag.equals(languageTag, ignoreCase = true)
        }

    private fun inferredSystemLanguage(): AppLanguage =
        if (Locale.getDefault().language.equals("zh", ignoreCase = true)) {
            AppLanguage.SIMPLIFIED_CHINESE
        } else {
            AppLanguage.ENGLISH
        }
}
