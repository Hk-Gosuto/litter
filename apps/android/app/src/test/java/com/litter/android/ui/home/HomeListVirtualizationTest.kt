package com.litter.android.ui.home

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class HomeListVirtualizationTest {
    @Test
    fun hydrationWindowCoversViewportPlusPrefetch() {
        assertEquals(0..17, HomeListVirtualization.hydrationWindow(0, 7, 1000))
        assertEquals(40..59, HomeListVirtualization.hydrationWindow(40, 55, 60))
        assertTrue(HomeListVirtualization.hydrationWindow(0, 0, 0).isEmpty())
    }

    @Test
    fun pagingGrowsByOnePageNearTheEnd() {
        assertEquals(50, HomeListVirtualization.renderedCount(50, 10, 1200))
        assertEquals(100, HomeListVirtualization.renderedCount(50, 40, 1200))
        assertEquals(1200, HomeListVirtualization.renderedCount(1200, 1195, 1200))
        assertEquals(30, HomeListVirtualization.renderedCount(50, 29, 30))
    }

}
