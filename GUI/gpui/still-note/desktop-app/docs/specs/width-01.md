# WIDTH-01: Full desktop window width

- Version:1
- Status:READY
- Spec owner:/root
- Base revision:8b93c6346fd34795425f02391ee18a1eeaae440f
- Request: Desktop app 화면 넓이를 모두 사용.

## Contract/AC
User request overrides DESIGN.md prior1280pxmax container only. Remove maximum centered width from navigation AND content/footer layout; retain24px outer horizontal padding (app inner content fills viewport minus48px). Sidebar/aside current widths preserved, central journal absorbs extra width. No new layout abstraction/dependency. Preserve Pretendard/color/buttoncontainment/mobile/shortwindow/scroll/journal state.

| AC | Expected | Verification |
|---|---|---|
| AC-01 | navigation content spans viewport, main design-container fullviewport, actual inner sidebar/body/aside/footer starts24px and endswidth-24px; no1280cap/large desktop gutters; journal expands as width increases | production bounds600/768/1024/1360/1600/1920/2560/2880, exactwidth/24pxgutter + journal growth; native hidden draw retained |
| AC-02 | resize wide to600x400 retains usable menu/scroll/forms/fixedbuttontext and data; sidepanels stable | existing longtext/shortheight/button/font E2E + responsive action tests wide |
| AC-03 | all journal/storage/IME contracts remain, README describes full-width currentpolicy | completeunit/integration/e2e + source/docs review |

## Ownership/verification
Builder/root/code src/ui.rs src/theme.rs README.md (removeunusedcapconstant if noactiveuse). Test/root/test tests/** artifacts/width-01/test*. Reviewer/root/reviewer artifacts/width-01/review*. Rootspec/final. ReadAGENTS/role/ponytail (alreadyused). graftcallers beforechanges and exhaustive maxwidth audit. TestwriteDONE then builderaloneGitBashformatwrite+explicitcheckpointincludespec. IndependentfixedcleanSHA fullGitBashverify5stages counts>0 no skips/weakening; sameSHA AC+review. Useexisting.artifacts/button-01/target output isolation if defaultstillnoteexe locked, reportenv. No personalapp shutdown. Native screenshot unavailable, headlesswidthgeometry vs actualnativepaint limit noted. Max3rework; allACsPASS+clean+independentowners forfinalPASS.
