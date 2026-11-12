package com.litter.android.ui.common

import androidx.annotation.StringRes
import com.sigkitten.litter.android.R

/**
 * The three session permission presets from the upstream permission menu.
 * Auto is on-request approval with workspace write. A null thread key stores
 * the choice as the pre-thread launch default; an existing thread stores a
 * user override for that session.
 */
internal enum class SessionPermissionChoice(
    val approvalPolicy: String,
    val sandboxMode: String,
    @StringRes val labelRes: Int,
    @StringRes val detailRes: Int,
) {
    READ_ONLY(
        "on-request",
        "read-only",
        R.string.permissions_read_only,
        R.string.permissions_menu_read_only_detail,
    ),
    AUTO(
        "on-request",
        "workspace-write",
        R.string.permissions_auto,
        R.string.permissions_menu_auto_detail,
    ),
    FULL_ACCESS(
        "never",
        "danger-full-access",
        R.string.permissions_full_access,
        R.string.permissions_menu_full_access_detail,
    ),
}
