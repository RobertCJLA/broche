// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.ui.theme

import android.app.Activity
import android.animation.ValueAnimator
import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.core.view.WindowCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner

val LocalReducedMotion = compositionLocalOf { false }
val LocalGlassTheme = compositionLocalOf { false }

/** True when Android's "Remove animations" (animator duration scale 0) is enabled. */
@Composable
fun rememberReducedMotion(): Boolean {
    var reduced by remember { mutableStateOf(!ValueAnimator.areAnimatorsEnabled()) }
    val lifecycleOwner = LocalLifecycleOwner.current
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) {
                reduced = !ValueAnimator.areAnimatorsEnabled()
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }
    return reduced
}

/** The user's look choices (see Preferences). */
@androidx.compose.runtime.Immutable
data class Appearance(
    val theme: String = "catppuccin",
    val mode: String = "system",
    val seed: String = Tokens.DEFAULT_SEED,
    /** Legacy preference retained when reading and migrating earlier themes. */
    val dynamicColor: Boolean = true,
    /** Legacy preference; current palettes determine surface colors. */
    val amoled: Boolean = false,
    /** Legacy preference; Hive is no longer offered. */
    val combMotion: Boolean = true,
)

private fun shapes() = Shapes(
    extraSmall = RoundedCornerShape(8.dp),
    small = RoundedCornerShape(12.dp),
    medium = RoundedCornerShape(16.dp),
    large = RoundedCornerShape(20.dp),
    extraLarge = RoundedCornerShape(28.dp),
)

/** Material 3 switch colors for all current themes. */
@Composable
fun brocheSwitchColors(): androidx.compose.material3.SwitchColors =
    androidx.compose.material3.SwitchDefaults.colors()

/** Material 3 theme with the current Broche palettes. */
@Composable
fun BrocheTheme(
    appearance: Appearance = Appearance(),
    content: @Composable () -> Unit,
) {
    val dark = when (appearance.mode) {
        "light" -> false
        "dark" -> true
        else -> isSystemInDarkTheme()
    }
    val context = LocalContext.current
    val baseScheme = remember(dark, appearance.theme, context) {
        if (appearance.theme == "system" && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            (if (dark) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)).accessible(dark)
        } else catppuccinScheme(dark, appearance.theme)
    }
    val targetScheme = baseScheme
    val reducedMotion = rememberReducedMotion()

    // System bar icons follow the app's theme, which can differ from the system's.
    val view = LocalView.current
    if (!view.isInEditMode) {
        androidx.compose.runtime.LaunchedEffect(view, dark) {
            val window = (view.context as? Activity)?.window ?: return@LaunchedEffect
            WindowCompat.getInsetsController(window, view).apply {
                isAppearanceLightStatusBars = !dark
                isAppearanceLightNavigationBars = !dark
            }
        }
    }
    val assets = context.assets
    val fonts = remember(assets) { AppFonts(assets) }
    val currentShapes = remember { shapes() }
    val currentTypography = remember(fonts) { typography(fonts) }
    CompositionLocalProvider(
        LocalAppFonts provides fonts,
        LocalReducedMotion provides reducedMotion,
    ) {
        MaterialTheme(
            colorScheme = targetScheme,
            shapes = currentShapes,
            typography = currentTypography,
        ) {
            content()
        }
    }
}

/**
 * Single-pass cached card background with a subtle uniform hairline border and no top-edge glare.
 */
fun Modifier.liquidGlassCard(
    cornerRadius: Dp = 24.dp,
    isGlass: Boolean,
    baseColor: Color,
    @Suppress("UNUSED_PARAMETER") tintColor: Color = Color.Transparent,
    outlineColor: Color = Color.Transparent,
): Modifier = this.composed {
    // Material outlined card: without an explicit outline, cards still get one, so they
    // never blend into a background of the same colour.
    val rim = if (outlineColor.alpha > 0f || isGlass) outlineColor else MaterialTheme.colorScheme.outlineVariant
    Modifier.cardBackground(cornerRadius, isGlass, baseColor, rim)
}

private fun Modifier.cardBackground(
    cornerRadius: Dp,
    isGlass: Boolean,
    baseColor: Color,
    outlineColor: Color,
): Modifier = this.drawWithCache {
    val radiusPx = cornerRadius.toPx()
    val cr = CornerRadius(radiusPx, radiusPx)
    val strokePx = 1.dp.toPx()
    val halfStroke = strokePx * 0.5f
    val topLeftOffset = Offset(halfStroke, halfStroke)
    val insetSize = Size(
        (size.width - strokePx).coerceAtLeast(0f),
        (size.height - strokePx).coerceAtLeast(0f),
    )
    val isDarkSurface = (baseColor.red * 0.299f + baseColor.green * 0.587f + baseColor.blue * 0.114f) < 0.5f
    val uniformRimColor = if (isGlass) {
        if (outlineColor.alpha > 0f) {
            outlineColor.copy(alpha = outlineColor.alpha.coerceAtMost(if (isDarkSurface) 0.14f else 0.22f))
        } else {
            Color.White.copy(alpha = if (isDarkSurface) 0.10f else 0.22f)
        }
    } else {
        outlineColor
    }
    val strokeStyle = Stroke(width = strokePx)
    onDrawBehind {
        drawRoundRect(color = baseColor, cornerRadius = cr)
        if (uniformRimColor.alpha > 0f) {
            drawRoundRect(
                color = uniformRimColor,
                topLeft = topLeftOffset,
                size = insetSize,
                cornerRadius = cr,
                style = strokeStyle,
            )
        }
    }
}

/**
 * No-op modifier kept for source compatibility; modal sheets use [glassModalScrimColor]
 * and [glassModalContainerColor] instead of blurring the entire root viewport into an FBO.
 */
fun Modifier.glassModalBlur(@Suppress("UNUSED_PARAMETER") modalOpen: Boolean): Modifier = this

/** Opaque Material surface for modal sheets and dialogs. */
@Composable
fun glassModalContainerColor(): Color = MaterialTheme.colorScheme.surfaceContainerHigh

/** Preserve the scrim opacity of the current Broche themes. */
@Composable
fun glassModalScrimColor(): Color = Color.Black.copy(alpha = 0.38f)

/**
 * Lightweight zero-RenderNode interior well for bento strips and inner rows.
 */
fun Modifier.bentoWell(
    cornerRadius: Dp = 16.dp,
    fillColor: Color,
    borderColor: Color = Color.Transparent,
): Modifier = this.drawWithCache {
    val r = cornerRadius.toPx()
    val cr = CornerRadius(r, r)
    val strokePx = 1.dp.toPx()
    val halfStroke = strokePx * 0.5f
    val topLeftOffset = Offset(halfStroke, halfStroke)
    val insetSize = Size(
        (size.width - strokePx).coerceAtLeast(0f),
        (size.height - strokePx).coerceAtLeast(0f),
    )
    val strokeStyle = Stroke(width = strokePx)
    onDrawBehind {
        drawRoundRect(color = fillColor, cornerRadius = cr)
        if (borderColor.alpha > 0f) {
            drawRoundRect(
                color = borderColor,
                topLeft = topLeftOffset,
                size = insetSize,
                cornerRadius = cr,
                style = strokeStyle,
            )
        }
    }
}

