"""Release signing for the iOS app extensions (RFC 0363).

The release job signs the app with one App Store profile by rewriting
every `CODE_SIGN_IDENTITY = "iPhone Developer";`. The extensions use
`"Apple Development"` so that rewrite never hands them the app's
profile. This script handles them on their own:

  sign  <pbxproj> <Target> <profile name> <profile uuid>
      Put the extension's own App Store profile on its configs.
  strip <pbxproj>
      Unhook every extension from the app (embed phase and target
      dependencies) and drop the App Group from the app's entitlements,
      so a release without the extension profiles (or with an app
      profile that lacks the App Group) still ships the app.
"""
import re
import sys
from pathlib import Path

EMBED = "0363E0000000000000000001"


def ext_targets(text):
    return re.findall(
        r"(\w{24}) /\* (\w+) \*/ = \{\n\t\t\tisa = PBXNativeTarget;(?:(?!\n\t\t\};).)*?"
        r'productType = "com\.apple\.product-type\.app-extension";',
        text,
        flags=re.S,
    )


def strip(text):
    text = text.replace(f"\t\t\t\t{EMBED} /* Embed Foundation Extensions */,\n", "")
    for tid, name in ext_targets(text):
        deps = re.findall(rf"(\w{{24}}) /\* PBXTargetDependency \*/ = \{{\n\t\t\tisa = PBXTargetDependency;\n\t\t\ttarget = {tid} ", text)
        for dep in deps:
            text = text.replace(f"\t\t\t\t{dep} /* PBXTargetDependency */,\n", "")
        print(f"::notice::unhooked {name} from the app")
    return text


def drop_app_group(plist):
    """Remove the application-groups key and its array from a plist."""
    out = re.sub(
        r"\s*<key>com\.apple\.security\.application-groups</key>\s*<array>.*?</array>",
        "",
        plist,
        flags=re.S,
    )
    print("::notice::dropped the App Group from the app entitlements")
    return out


def sign(text, target, name, uuid):
    pattern = re.compile(
        rf'(CODE_SIGN_ENTITLEMENTS = {target}/{target}\.entitlements;\n\t+)CODE_SIGN_IDENTITY = "Apple Development";'
    )
    repl = (
        r'\1CODE_SIGN_IDENTITY = "Apple Distribution"; CODE_SIGN_STYLE = Manual; '
        f'PROVISIONING_PROFILE_SPECIFIER = "{name}"; PROVISIONING_PROFILE = "{uuid}";'
    )
    text, n = pattern.subn(repl, text)
    if n == 0:
        sys.exit(f"::error::no signing block found for {target}")
    print(f"::notice::signed {n} config(s) of {target}")
    return text


def main(argv):
    mode, pbx = argv[1], Path(argv[2])
    text = pbx.read_text()
    if mode == "strip":
        text = strip(text)
        ent = pbx.parent.parent / "outl-mobile_iOS" / "outl-mobile_iOS.entitlements"
        if ent.exists():
            ent.write_text(drop_app_group(ent.read_text()))
    elif mode == "sign":
        text = sign(text, argv[3], argv[4], argv[5])
    else:
        sys.exit(f"unknown mode {mode}")
    pbx.write_text(text)


if __name__ == "__main__":
    main(sys.argv)
