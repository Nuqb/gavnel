# Comparison notes

[Back to the README](../README.md)

- **Gavnel:** local files; no ads, paid tier, cloud uploads, or account requirement; standalone native app; **86 selectable language and regional options** ([localization notes](localization.md)).
- **Medal:** [local files][medal-local]; [free-tier ads, Premium, and cloud options][medal-plans]; [account required][medal-account]; standalone app; **22 language options** in its [official supported-language list][medal-languages].
- **Outplayed:** [local files][outplayed-local]; [free-tier ads, Premium, and optional hosting][outplayed-plans]; [Overwolf client required][outplayed-app]. [Account required to access Outplayed][outplayed-plans], though recording continues while logged out. **English** is the sole text/interface language listed in its [publisher-provided Epic Games Store listing][outplayed-languages].

**Language options** counts selectable interface locales, including regional and writing-system variants. Gavnel's 86 choices cover 81 base language codes; 14 additional draft catalogs are withheld. Selectable translations are initial AI-assisted work awaiting independent native-speaker review. Medal's 22-item list includes Simplified and Traditional Chinese separately. Outplayed's count reflects its published store listing. Counts describe available options rather than equivalent translation quality.

Official product icons: [Medal support](https://support.medal.tv/) and [Outplayed](https://outplayed.tv/). Their marks belong to their respective owners and identify the products being compared.

Comparison checked **November 9, 2026**, using the vendors' official documentation. Plans and features can change. Gavnel's column describes this repository's implementation and stated direction, rather than a promise of future features.

Medal documents [slow interface/editor cases][medal-ui] and [choppy-recording troubleshooting][medal-performance]. Overwolf documents [capture performance issues][outplayed-performance] too. These describe problems that can occur; they do not establish that either app is always slow.

Fast startup and low recording overhead are Gavnel's goals. **There is no side-by-side startup, FPS, or memory benchmark here.** Both alternatives also support hardware encoding. Actual results depend on the game, hardware, driver, recording settings, and storage.

Gavnel currently has a narrower feature set: Windows with an NVIDIA GPU, a built-in game-detection catalog, and no automatic kill/death highlight detection, cloud sync, or built-in montage editor.

[medal-plans]: https://support.medal.tv/support/solutions/articles/48001218475-medal-premium-faq
[medal-local]: https://support.medal.tv/support/solutions/articles/48001227747
[medal-performance]: https://support.medal.tv/support/solutions/articles/48000922094
[medal-ui]: https://support.medal.tv/support/solutions/articles/48001242234-nvidia-issues-with-desktop-app-editor-ui-blurry-slow-
[outplayed-app]: https://www.overwolf.com/app/overwolf-outplayed
[outplayed-plans]: https://support.overwolf.com/support/solutions/articles/9000284652-outplayed-account-plans
[outplayed-guide]: https://support.overwolf.com/support/solutions/articles/9000284663-outplayed-quick-set-up-guide
[outplayed-local]: https://support.overwolf.com/support/solutions/articles/9000284626-outplayed-faq
[outplayed-performance]: https://support.overwolf.com/support/solutions/articles/9000181387-common-capture-issues
[overwolf-codecs]: https://support.overwolf.com/support/solutions/articles/9000177861-video-codec-selection-and-availability

[medal-account]: https://support.medal.tv/support/solutions/articles/48001147502/

[medal-languages]: https://support.medal.tv/support/solutions/articles/48001280799-community-programs-faq
[outplayed-languages]: https://store.epicgames.com/p/outplayed/?lang=en-US
