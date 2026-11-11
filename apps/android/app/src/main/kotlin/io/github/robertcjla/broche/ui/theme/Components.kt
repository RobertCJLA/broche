// SPDX-License-Identifier: GPL-3.0-or-later
package io.github.robertcjla.broche.ui.theme

import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.robertcjla.broche.R

/**
 * The top bar of a screen opened from Home (Material "small top app bar"): back arrow,
 * title, and the connection written out (with a dot), so colour is not the only signal.
 */
@Composable
fun ScreenHeader(
    title: String,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
    online: Boolean? = null,
    subtitle: String? = null,
    actions: @Composable RowScope.() -> Unit = {},
) {
    val scheme = MaterialTheme.colorScheme
    Row(
        modifier = modifier.fillMaxWidth().heightIn(min = 64.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = onBack) {
            Icon(painterResource(R.drawable.ic_back), contentDescription = stringResource(R.string.action_back))
        }
        Spacer(Modifier.width(4.dp))
        Column(Modifier.weight(1f)) {
            Text(
                title,
                style = MaterialTheme.typography.titleLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.semantics { heading() },
            )
            if (online != null) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    Surface(shape = CircleShape, color = if (online) scheme.primary else scheme.outline, modifier = Modifier.size(8.dp)) {}
                    Text(
                        stringResource(if (online) R.string.connected else R.string.not_connected),
                        style = MaterialTheme.typography.bodySmall,
                        color = scheme.onSurfaceVariant,
                    )
                }
            }
            if (subtitle != null) {
                Text(
                    subtitle,
                    style = MaterialTheme.typography.bodySmall,
                    color = scheme.onSurfaceVariant,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        actions()
    }
}

/** A Material search field (full rounded, with a clear button). */
@Composable
fun BrocheSearchField(query: String, onQuery: (String) -> Unit, hint: String, modifier: Modifier = Modifier) {
    val scheme = MaterialTheme.colorScheme
    val shape = RoundedCornerShape(28.dp)
    TextField(
        value = query,
        onValueChange = onQuery,
        singleLine = true,
        placeholder = { Text(hint) },
        leadingIcon = { Icon(painterResource(R.drawable.ic_search), contentDescription = null) },
        trailingIcon = if (query.isNotEmpty()) {
            {
                IconButton(onClick = { onQuery("") }) {
                    Icon(painterResource(R.drawable.ic_close), contentDescription = stringResource(R.string.settings_search_clear))
                }
            }
        } else {
            null
        },
        shape = shape,
        colors = TextFieldDefaults.colors(
            focusedContainerColor = scheme.surfaceContainerHigh,
            unfocusedContainerColor = scheme.surfaceContainerHigh,
            focusedIndicatorColor = Color.Transparent,
            unfocusedIndicatorColor = Color.Transparent,
        ),
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        modifier = modifier
            .fillMaxWidth()
            .border(1.dp, scheme.outline, shape)
            .semantics { contentDescription = hint },
    )
}

/** A short status label in a tonal container, with an icon so colour isn't the only signal. */
@Composable
fun StatusLabel(text: String, ok: Boolean, modifier: Modifier = Modifier) {
    val scheme = MaterialTheme.colorScheme
    Surface(shape = RoundedCornerShape(8.dp), color = if (ok) scheme.tertiaryContainer else scheme.secondaryContainer, modifier = modifier) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp),
        ) {
            Icon(
                painterResource(if (ok) R.drawable.ic_check_circle else R.drawable.ic_info),
                contentDescription = null,
                tint = if (ok) scheme.onTertiaryContainer else scheme.onSecondaryContainer,
                modifier = Modifier.size(16.dp),
            )
            Text(
                text,
                style = MaterialTheme.typography.labelMedium,
                color = if (ok) scheme.onTertiaryContainer else scheme.onSecondaryContainer,
            )
        }
    }
}

/**
 * A button's text: up to two centred lines, so a long label (Spanish, large text) wraps
 * instead of being cut. Pair it with `heightIn(min = …)` on the button, not a fixed height.
 */
@Composable
fun ButtonLabel(text: String, modifier: Modifier = Modifier) {
    Text(
        text,
        modifier = modifier,
        style = MaterialTheme.typography.labelLarge,
        textAlign = TextAlign.Center,
        maxLines = 2,
        overflow = TextOverflow.Ellipsis,
    )
}
