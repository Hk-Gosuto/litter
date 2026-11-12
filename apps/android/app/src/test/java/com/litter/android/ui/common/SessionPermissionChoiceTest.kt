package com.litter.android.ui.common

import com.sigkitten.litter.android.R
import org.junit.Assert.assertEquals
import org.junit.Test

class SessionPermissionChoiceTest {
    @Test
    fun presetsMatchUpstreamReadOnlyAutoAndFullAccess() {
        assertEquals("on-request", SessionPermissionChoice.READ_ONLY.approvalPolicy)
        assertEquals("read-only", SessionPermissionChoice.READ_ONLY.sandboxMode)
        assertEquals("on-request", SessionPermissionChoice.AUTO.approvalPolicy)
        assertEquals("workspace-write", SessionPermissionChoice.AUTO.sandboxMode)
        assertEquals("never", SessionPermissionChoice.FULL_ACCESS.approvalPolicy)
        assertEquals("danger-full-access", SessionPermissionChoice.FULL_ACCESS.sandboxMode)
        assertEquals(R.string.permissions_menu_read_only_detail, SessionPermissionChoice.READ_ONLY.detailRes)
        assertEquals(R.string.permissions_menu_auto_detail, SessionPermissionChoice.AUTO.detailRes)
        assertEquals(R.string.permissions_menu_full_access_detail, SessionPermissionChoice.FULL_ACCESS.detailRes)
        assertEquals(
            listOf(
                SessionPermissionChoice.READ_ONLY,
                SessionPermissionChoice.AUTO,
                SessionPermissionChoice.FULL_ACCESS,
            ),
            SessionPermissionChoice.entries,
        )
    }
}
