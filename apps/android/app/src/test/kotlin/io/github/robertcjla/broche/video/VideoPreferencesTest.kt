package io.github.robertcjla.broche.video
import org.junit.Assert.assertEquals
import org.junit.Test
class VideoPreferencesTest {
    @Test fun normalVideoNeverExceedsThirty() {
        assertEquals(30, VideoPreferences.cap(120, false))
        assertEquals(24, VideoPreferences.cap(24, false))
        assertEquals(1, VideoPreferences.cap(0, false))
    }
    @Test fun sixtyRequiresExperimentalModeAndStillHasACeiling() {
        assertEquals(60, VideoPreferences.cap(120, true))
        assertEquals(30, VideoPreferences.cap(30, true))
    }
}
