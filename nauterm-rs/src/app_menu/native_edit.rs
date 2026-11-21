use objc2::{MainThreadMarker, rc::Retained, runtime::Sel, sel};
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem};
use objc2_foundation::NSString;

use super::menu_title;

type Title = (&'static str, &'static str);
type ResponderCommand = (Title, Sel, &'static str);

pub(super) const EDIT: Title = ("5QF-Oa-p0T.title", "Edit");
pub(super) const FIND: Title = ("4EN-yA-p0u.title", "Find");
pub(super) const PASTE_STYLE: Title = ("WeT-3V-zwk.title", "Paste and Match Style");

fn title((key, fallback): Title) -> Retained<NSString> {
    menu_title(key, fallback)
}

fn responder_item(
    main: MainThreadMarker,
    (label, action, key_equivalent): ResponderCommand,
) -> Retained<NSMenuItem> {
    let item = NSMenuItem::new(main);
    item.setTitle(&title(label));
    item.setKeyEquivalent(&NSString::from_str(key_equivalent));
    // These AppKit action selectors take a sender. Leave the target nil so
    // AppKit selects and validates the current responder when opening the menu.
    unsafe { item.setAction(Some(action)) };
    item
}

fn add_submenu(main: MainThreadMarker, edit: &NSMenu, label: Title, entries: &[ResponderCommand]) {
    let item = NSMenuItem::new(main);
    let title = title(label);
    let submenu = NSMenu::new(main);
    submenu.setTitle(&title);
    for &entry in entries {
        submenu.addItem(&responder_item(main, entry));
    }
    item.setTitle(&title);
    item.setSubmenu(Some(&submenu));
    edit.addItem(&item);
}

pub(super) fn install() {
    let Some(main) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(main);
    let Some(edit) = app
        .mainMenu()
        .and_then(|menu| menu.itemWithTitle(&title(EDIT)))
        .and_then(|item| item.submenu())
    else {
        return;
    };

    // GPUI inputs already paste plain text, so both entries share its Paste
    // action. Give the additional item the standard alternate key equivalent.
    if let Some(item) = edit.itemWithTitle(&title(PASTE_STYLE)) {
        item.setKeyEquivalent(&NSString::from_str("v"));
        item.setKeyEquivalentModifierMask(
            NSEventModifierFlags::Command
                | NSEventModifierFlags::Option
                | NSEventModifierFlags::Shift,
        );
    }

    if let Some(find) = edit
        .itemWithTitle(&title(FIND))
        .and_then(|item| item.submenu())
    {
        let replace = responder_item(
            main,
            (
                ("YEy-JH-Tfz.title", "Find and Replace…"),
                sel!(performTextFinderAction:),
                "f",
            ),
        );
        // NSTextFinderActionShowReplaceInterface
        replace.setTag(12);
        replace.setKeyEquivalentModifierMask(
            NSEventModifierFlags::Command | NSEventModifierFlags::Option,
        );
        find.insertItem_atIndex(&replace, 1);
    }

    edit.addItem(&NSMenuItem::separatorItem(main));
    add_submenu(
        main,
        &edit,
        ("Dv1-io-Yv7.title", "Spelling and Grammar"),
        &[
            (
                ("HFo-cy-zxI.title", "Show Spelling and Grammar"),
                sel!(showGuessPanel:),
                ":",
            ),
            (
                ("hz2-CU-CR7.title", "Check Document Now"),
                sel!(checkSpelling:),
                ";",
            ),
            (
                ("rbD-Rh-wIN.title", "Check Spelling While Typing"),
                sel!(toggleContinuousSpellChecking:),
                "",
            ),
            (
                ("mK6-2p-4JG.title", "Check Grammar With Spelling"),
                sel!(toggleGrammarChecking:),
                "",
            ),
            (
                ("78Y-hA-62v.title", "Correct Spelling Automatically"),
                sel!(toggleAutomaticSpellingCorrection:),
                "",
            ),
        ],
    );
    add_submenu(
        main,
        &edit,
        ("9ic-FL-obx.title", "Substitutions"),
        &[
            (
                ("z6F-FW-3nz.title", "Show Substitutions"),
                sel!(orderFrontSubstitutionsPanel:),
                "",
            ),
            (
                ("9yt-4B-nSM.title", "Smart Copy/Paste"),
                sel!(toggleSmartInsertDelete:),
                "",
            ),
            (
                ("hQb-2v-fYv.title", "Smart Quotes"),
                sel!(toggleAutomaticQuoteSubstitution:),
                "",
            ),
            (
                ("rgM-f4-ycn.title", "Smart Dashes"),
                sel!(toggleAutomaticDashSubstitution:),
                "",
            ),
            (
                ("cwL-P1-jid.title", "Smart Links"),
                sel!(toggleAutomaticLinkDetection:),
                "",
            ),
            (
                ("tRr-pd-1PS.title", "Data Detectors"),
                sel!(toggleAutomaticDataDetection:),
                "",
            ),
            (
                ("HFQ-gK-NFA.title", "Text Replacement"),
                sel!(toggleAutomaticTextReplacement:),
                "",
            ),
        ],
    );
    add_submenu(
        main,
        &edit,
        ("2oI-Rn-ZJC.title", "Transformations"),
        &[
            (
                ("vmV-6d-7jI.title", "Make Upper Case"),
                sel!(uppercaseWord:),
                "",
            ),
            (
                ("d9M-CD-aMd.title", "Make Lower Case"),
                sel!(lowercaseWord:),
                "",
            ),
            (
                ("UEZ-Bs-lqG.title", "Capitalize"),
                sel!(capitalizeWord:),
                "",
            ),
        ],
    );
    add_submenu(
        main,
        &edit,
        ("xrE-MZ-jX0.title", "Speech"),
        &[
            (
                ("Ynk-f8-cLZ.title", "Start Speaking"),
                sel!(startSpeaking:),
                "",
            ),
            (
                ("Oyz-dy-DGm.title", "Stop Speaking"),
                sel!(stopSpeaking:),
                "",
            ),
        ],
    );
}
