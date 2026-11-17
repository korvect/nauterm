import 'package:material_ui/material_ui.dart';

import '../terminal/terminal_theme.dart';
import 'nauterm_typography.dart';

bool isDarkColor(Color color) {
  return color.computeLuminance() < 0.35;
}

List<List<Color>> themePreviewSwatches(TerminalTheme theme) {
  return [
    [
      theme.normal.black,
      theme.normal.red,
      theme.normal.green,
      theme.normal.yellow,
      theme.normal.blue,
      theme.normal.magenta,
      theme.normal.cyan,
      theme.normal.white,
    ],
    [
      theme.bright.black,
      theme.bright.red,
      theme.bright.green,
      theme.bright.yellow,
      theme.bright.blue,
      theme.bright.magenta,
      theme.bright.cyan,
      theme.bright.white,
    ],
  ];
}

class TerminalThemePreviewCard extends StatelessWidget {
  const TerminalThemePreviewCard({
    super.key,
    required this.title,
    required this.theme,
    this.selected = false,
    this.compact = false,
    this.onTap,
  });

  final String title;
  final TerminalTheme theme;
  final bool selected;
  final bool compact;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final surface = theme.primary.background;
    final foreground = theme.primary.foreground;
    final accent = theme.primary.accent;
    final cursor = theme.cursor.cursor;
    final borderColor = selected
        ? Color.alphaBlend(accent.withValues(alpha: 0.72), surface)
        : Color.alphaBlend(foreground.withValues(alpha: 0.13), surface);
    final shadowColor = isDarkColor(surface)
        ? Colors.black.withValues(alpha: 0.16)
        : Colors.black.withValues(alpha: selected ? 0.08 : 0.045);

    final card = AnimatedContainer(
      duration: const Duration(milliseconds: 120),
      curve: Curves.easeOutCubic,
      decoration: BoxDecoration(
        color: surface,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: borderColor),
        boxShadow: [
          BoxShadow(
            color: shadowColor,
            blurRadius: selected ? 16 : 10,
            offset: const Offset(0, 5),
          ),
        ],
      ),
      padding: EdgeInsets.fromLTRB(
        12,
        compact ? 10 : 11,
        12,
        compact ? 10 : 12,
      ),
      child: Stack(
        children: [
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              SizedBox(
                height: compact ? 16 : 31,
                child: Padding(
                  padding: const EdgeInsets.only(right: 14),
                  child: Text(
                    title,
                    maxLines: compact ? 1 : 2,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: foreground,
                      fontSize: compact
                          ? NautermFontSizes.labelLarge
                          : NautermFontSizes.labelMedium,
                      fontWeight: NautermFontWeights.semibold,
                      letterSpacing: 0,
                      height: 1.08,
                    ),
                  ),
                ),
              ),
              const SizedBox(height: 9),
              Container(
                key: const ValueKey('terminal-theme-preview-sample'),
                height: compact ? 34 : 38,
                decoration: BoxDecoration(
                  color: surface,
                  borderRadius: BorderRadius.circular(6),
                  border: Border.all(
                    color: Color.alphaBlend(
                      foreground.withValues(alpha: 0.08),
                      surface,
                    ),
                  ),
                ),
                padding: const EdgeInsets.symmetric(horizontal: 10),
                child: LayoutBuilder(
                  builder: (context, constraints) {
                    final showSelection = constraints.maxWidth >= 64;
                    final showCursor =
                        constraints.maxWidth >= (showSelection ? 28 : 12);

                    return Row(
                      children: [
                        Expanded(
                          child: Row(
                            children: [
                              Flexible(
                                child: Text(
                                  key: const ValueKey(
                                    'terminal-theme-preview-command',
                                  ),
                                  r'$ ls -la',
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: TextStyle(
                                    color: foreground,
                                    fontSize: NautermFontSizes.labelSmall,
                                    fontWeight: NautermFontWeights.medium,
                                    letterSpacing: 0,
                                    fontFamily: 'monospace',
                                  ),
                                ),
                              ),
                              if (showCursor) ...[
                                const SizedBox(width: 5),
                                Container(
                                  key: const ValueKey(
                                    'terminal-theme-preview-cursor',
                                  ),
                                  width: 6,
                                  height: 13,
                                  decoration: BoxDecoration(
                                    color: cursor,
                                    borderRadius: BorderRadius.circular(2),
                                  ),
                                ),
                              ],
                            ],
                          ),
                        ),
                        if (showSelection)
                          Container(
                            padding: const EdgeInsets.symmetric(
                              horizontal: 7,
                              vertical: 3,
                            ),
                            decoration: BoxDecoration(
                              color: theme.selection.background,
                              borderRadius: BorderRadius.circular(5),
                            ),
                            child: Text(
                              'sel',
                              style: TextStyle(
                                color: theme.selection.text,
                                fontSize: 10,
                                fontWeight: NautermFontWeights.medium,
                                letterSpacing: 0,
                              ),
                            ),
                          ),
                      ],
                    );
                  },
                ),
              ),
              const SizedBox(height: 8),
              SizedBox(height: 16, child: ThemePreviewSwatches(theme: theme)),
            ],
          ),
          Positioned(
            top: 1,
            right: 1,
            child: AnimatedContainer(
              duration: const Duration(milliseconds: 120),
              width: selected ? 11 : 9,
              height: selected ? 11 : 9,
              decoration: BoxDecoration(
                color: accent,
                shape: BoxShape.circle,
                border: Border.all(
                  color: Color.alphaBlend(
                    surface.withValues(alpha: 0.68),
                    accent,
                  ),
                ),
              ),
            ),
          ),
        ],
      ),
    );
    if (onTap == null) return card;
    return RepaintBoundary(
      child: Material(
        color: Colors.transparent,
        borderRadius: BorderRadius.circular(8),
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          onTap: onTap,
          child: card,
        ),
      ),
    );
  }
}

class ThemePreviewSwatches extends StatelessWidget {
  const ThemePreviewSwatches({super.key, required this.theme});

  final TerminalTheme theme;

  @override
  Widget build(BuildContext context) {
    final rows = themePreviewSwatches(theme);

    return LayoutBuilder(
      builder: (context, constraints) {
        const verticalGap = 4.0;
        final maxCount = rows.fold<int>(
          0,
          (count, row) => row.length > count ? row.length : count,
        );
        final horizontalGap = maxCount <= 1
            ? 0.0
            : (constraints.maxWidth / (maxCount * 5)).clamp(0.0, 4.0);
        final swatchWidth =
            ((constraints.maxWidth - horizontalGap * (maxCount - 1)) / maxCount)
                .clamp(0.0, 12.0);

        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            for (var rowIndex = 0; rowIndex < rows.length; rowIndex++) ...[
              Row(
                children: [
                  for (
                    var colorIndex = 0;
                    colorIndex < rows[rowIndex].length;
                    colorIndex++
                  ) ...[
                    Container(
                      width: swatchWidth,
                      height: 6,
                      decoration: BoxDecoration(
                        color: rows[rowIndex][colorIndex],
                        borderRadius: BorderRadius.circular(3),
                      ),
                    ),
                    if (colorIndex != rows[rowIndex].length - 1)
                      SizedBox(width: horizontalGap),
                  ],
                ],
              ),
              if (rowIndex != rows.length - 1)
                const SizedBox(height: verticalGap),
            ],
          ],
        );
      },
    );
  }
}
