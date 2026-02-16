package com.zerodevi1.chaos_seed.ui.components

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.width
import androidx.compose.material3.ScrollableTabRow
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

@Composable
fun LiveSiteTabs(
    sites: List<LiveSite>,
    selectedIndex: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
    iconSize: Dp = 20.dp,
) {
    ScrollableTabRow(
        selectedTabIndex = selectedIndex,
        modifier = modifier,
        edgePadding = 8.dp,
    ) {
        sites.forEachIndexed { idx, s ->
            Tab(
                selected = selectedIndex == idx,
                onClick = { onSelect(idx) },
                text = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Image(
                            painter = painterResource(s.iconRes),
                            contentDescription = null,
                            modifier = Modifier.size(iconSize),
                        )
                        Spacer(Modifier.width(8.dp))
                        Text(s.label)
                    }
                },
            )
        }
    }
}
