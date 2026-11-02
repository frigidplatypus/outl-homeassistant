"""Add an iOS app-extension target to outl-mobile's project.pbxproj.

Usage: add_ext.py <pbxproj> <name> <folder> <bundle_suffix> <prefix6> <deploy> <swift files...>
Inserts: file refs, group, sources/frameworks/resources phases, OutlKit
product dependency, native target, configs, embed phase + dependency on
the app target. Idempotent guard: refuses when the target already exists.
"""
import sys, re

pbx, name, folder, suffix, prefix, deploy, *swift = sys.argv[1:]
s = open(pbx).read()
if f'/* {name} */ = {{\n\t\t\tisa = PBXNativeTarget' in s:
    sys.exit(f"{name} already present")
n = [0]
def nid():
    n[0] += 1
    return f"{prefix}{n[0]:018X}"

APP = "1A9BDF647CE25CAD1577771B"
PKG = "4F2E9D183B5A4C6E91A2B3C4"
PROJ = "D189CF478257466E80C23D09"
appex = f"{name}.appex"
ids = {k: nid() for k in "product plist ent group src fw res target cfgdbg cfgrel cfglist pkgdep pkgbf embedbf dep proxy".split()}
sw = [(f, nid(), nid()) for f in swift]  # (file, fileref, buildfile)

def ins(section, text):
    global s
    marker = f"/* End {section} section */"
    if marker not in s:
        s = s.replace("/* Begin XCBuildConfiguration section */",
                      f"/* Begin {section} section */\n/* End {section} section */\n\n/* Begin XCBuildConfiguration section */")
    s = s.replace(marker, text + marker, 1)

ins("PBXBuildFile", "".join(
    f"\t\t{b} /* {f} in Sources */ = {{isa = PBXBuildFile; fileRef = {r} /* {f} */; }};\n" for f, r, b in sw)
    + f"\t\t{ids['pkgbf']} /* OutlKit in Frameworks */ = {{isa = PBXBuildFile; productRef = {ids['pkgdep']} /* OutlKit */; }};\n"
    + f"\t\t{ids['embedbf']} /* {appex} in Embed Foundation Extensions */ = {{isa = PBXBuildFile; fileRef = {ids['product']} /* {appex} */; settings = {{ATTRIBUTES = (RemoveHeadersOnCopy, ); }}; }};\n")

ins("PBXFileReference", "".join(
    f"\t\t{r} /* {f} */ = {{isa = PBXFileReference; lastKnownFileType = sourcecode.swift; path = {f}; sourceTree = \"<group>\"; }};\n" for f, r, b in sw)
    + f"\t\t{ids['plist']} /* Info.plist */ = {{isa = PBXFileReference; lastKnownFileType = text.plist; path = Info.plist; sourceTree = \"<group>\"; }};\n"
    + f"\t\t{ids['ent']} /* {name}.entitlements */ = {{isa = PBXFileReference; lastKnownFileType = text.plist.entitlements; path = {name}.entitlements; sourceTree = \"<group>\"; }};\n"
    + f"\t\t{ids['product']} /* {appex} */ = {{isa = PBXFileReference; explicitFileType = \"wrapper.app-extension\"; includeInIndex = 0; path = {appex}; sourceTree = BUILT_PRODUCTS_DIR; }};\n")

ins("PBXGroup", f"""\t\t{ids['group']} /* {folder} */ = {{
\t\t\tisa = PBXGroup;
\t\t\tchildren = (
""" + "".join(f"\t\t\t\t{r} /* {f} */,\n" for f, r, b in sw) + f"""\t\t\t\t{ids['plist']} /* Info.plist */,
\t\t\t\t{ids['ent']} /* {name}.entitlements */,
\t\t\t);
\t\t\tpath = {folder};
\t\t\tsourceTree = "<group>";
\t\t}};
""")
# main group + products
assert "\t\t\t\t00ADA1A6367B132C265E551F /* outl-mobile_iOS */,\n" in s
s = s.replace("\t\t\t\t00ADA1A6367B132C265E551F /* outl-mobile_iOS */,\n",
              f"\t\t\t\t00ADA1A6367B132C265E551F /* outl-mobile_iOS */,\n\t\t\t\t{ids['group']} /* {folder} */,\n", 1)
s = s.replace("\t\t\t\t6B1DBE6C01DBA911F071B23C /* outl.app */,\n",
              f"\t\t\t\t6B1DBE6C01DBA911F071B23C /* outl.app */,\n\t\t\t\t{ids['product']} /* {appex} */,\n", 1)

phase = lambda pid, isa, label, files: f"""\t\t{pid} /* {label} */ = {{
\t\t\tisa = {isa};
\t\t\tbuildActionMask = 2147483647;
\t\t\tfiles = (
{files}\t\t\t);
\t\t\trunOnlyForDeploymentPostprocessing = 0;
\t\t}};
"""
ins("PBXSourcesBuildPhase", phase(ids['src'], "PBXSourcesBuildPhase", "Sources",
    "".join(f"\t\t\t\t{b} /* {f} in Sources */,\n" for f, r, b in sw)))
ins("PBXFrameworksBuildPhase", phase(ids['fw'], "PBXFrameworksBuildPhase", "Frameworks",
    f"\t\t\t\t{ids['pkgbf']} /* OutlKit in Frameworks */,\n"))
ins("PBXResourcesBuildPhase", phase(ids['res'], "PBXResourcesBuildPhase", "Resources", ""))

# embed phase on the app (shared across extensions)
EMBED = "0363E0000000000000000001"
if EMBED not in s:
    ins("PBXCopyFilesBuildPhase", f"""\t\t{EMBED} /* Embed Foundation Extensions */ = {{
\t\t\tisa = PBXCopyFilesBuildPhase;
\t\t\tbuildActionMask = 2147483647;
\t\t\tdstPath = "";
\t\t\tdstSubfolderSpec = 13;
\t\t\tfiles = (
\t\t\t);
\t\t\tname = "Embed Foundation Extensions";
\t\t\trunOnlyForDeploymentPostprocessing = 0;
\t\t}};
""")
    s = s.replace("\t\t\t\t0CBA679F725933061BBB5E3D /* Frameworks */,\n\t\t\t);\n\t\t\tbuildRules",
                  f"\t\t\t\t0CBA679F725933061BBB5E3D /* Frameworks */,\n\t\t\t\t{EMBED} /* Embed Foundation Extensions */,\n\t\t\t);\n\t\t\tbuildRules", 1)
s = re.sub(rf"({EMBED} /\* Embed Foundation Extensions \*/ = \{{.*?files = \(\n)",
           lambda m: m.group(1) + f"\t\t\t\t{ids['embedbf']} /* {appex} in Embed Foundation Extensions */,\n", s, count=1, flags=re.S)

ins("PBXContainerItemProxy", f"""\t\t{ids['proxy']} /* PBXContainerItemProxy */ = {{
\t\t\tisa = PBXContainerItemProxy;
\t\t\tcontainerPortal = {PROJ} /* Project object */;
\t\t\tproxyType = 1;
\t\t\tremoteGlobalIDString = {ids['target']};
\t\t\tremoteInfo = {name};
\t\t}};
""")
ins("PBXTargetDependency", f"""\t\t{ids['dep']} /* PBXTargetDependency */ = {{
\t\t\tisa = PBXTargetDependency;
\t\t\ttarget = {ids['target']} /* {name} */;
\t\t\ttargetProxy = {ids['proxy']} /* PBXContainerItemProxy */;
\t\t}};
""")
# app target dependencies
s = re.sub(rf"({APP} /\* outl-mobile_iOS \*/ = \{{.*?dependencies = \(\n)",
           lambda m: m.group(1) + f"\t\t\t\t{ids['dep']} /* PBXTargetDependency */,\n", s, count=1, flags=re.S)

ins("PBXNativeTarget", f"""\t\t{ids['target']} /* {name} */ = {{
\t\t\tisa = PBXNativeTarget;
\t\t\tbuildConfigurationList = {ids['cfglist']} /* Build configuration list for PBXNativeTarget "{name}" */;
\t\t\tbuildPhases = (
\t\t\t\t{ids['src']} /* Sources */,
\t\t\t\t{ids['fw']} /* Frameworks */,
\t\t\t\t{ids['res']} /* Resources */,
\t\t\t);
\t\t\tbuildRules = (
\t\t\t);
\t\t\tdependencies = (
\t\t\t);
\t\t\tname = {name};
\t\t\tpackageProductDependencies = (
\t\t\t\t{ids['pkgdep']} /* OutlKit */,
\t\t\t);
\t\t\tproductName = {name};
\t\t\tproductReference = {ids['product']} /* {appex} */;
\t\t\tproductType = "com.apple.product-type.app-extension";
\t\t}};
""")
s, hits = re.subn(r"(\n\t\t\ttargets = \(\n(?:\t\t\t\t\w{24} /\* [^*]+ \*/,\n)*)(\t\t\t\);)",
                 lambda m: m.group(1) + f"\t\t\t\t{ids['target']} /* {name} */,\n" + m.group(2), s, count=1)
assert hits == 1, "project targets list not found"

def cfg(cid, cname):
    return f"""\t\t{cid} /* {cname} */ = {{
\t\t\tisa = XCBuildConfiguration;
\t\t\tbuildSettings = {{
\t\t\t\tCODE_SIGN_ENTITLEMENTS = {folder}/{name}.entitlements;
\t\t\t\tCODE_SIGN_IDENTITY = "Apple Development";
\t\t\t\tCURRENT_PROJECT_VERSION = 1.0.0;
\t\t\t\tDEVELOPMENT_TEAM = CPEEKT3E77;
\t\t\t\tGENERATE_INFOPLIST_FILE = NO;
\t\t\t\tINFOPLIST_FILE = {folder}/Info.plist;
\t\t\t\tIPHONEOS_DEPLOYMENT_TARGET = {deploy};
\t\t\t\tLD_RUNPATH_SEARCH_PATHS = (
\t\t\t\t\t"$(inherited)",
\t\t\t\t\t"@executable_path/Frameworks",
\t\t\t\t\t"@executable_path/../../Frameworks",
\t\t\t\t);
\t\t\t\tMARKETING_VERSION = 0.2.0;
\t\t\t\tPRODUCT_BUNDLE_IDENTIFIER = "app.outl.mobile-app.{suffix}";
\t\t\t\tPRODUCT_NAME = "$(TARGET_NAME)";
\t\t\t\tSDKROOT = iphoneos;
\t\t\t\tSKIP_INSTALL = YES;
\t\t\t\tSWIFT_EMIT_LOC_STRINGS = YES;
\t\t\t\tSWIFT_VERSION = 5.0;
\t\t\t\tTARGETED_DEVICE_FAMILY = "1,2";
\t\t\t}};
\t\t\tname = {cname};
\t\t}};
"""
ins("XCBuildConfiguration", cfg(ids['cfgdbg'], "debug") + cfg(ids['cfgrel'], "release"))
ins("XCConfigurationList", f"""\t\t{ids['cfglist']} /* Build configuration list for PBXNativeTarget "{name}" */ = {{
\t\t\tisa = XCConfigurationList;
\t\t\tbuildConfigurations = (
\t\t\t\t{ids['cfgdbg']} /* debug */,
\t\t\t\t{ids['cfgrel']} /* release */,
\t\t\t);
\t\t\tdefaultConfigurationIsVisible = 0;
\t\t\tdefaultConfigurationName = debug;
\t\t}};
""")
ins("XCSwiftPackageProductDependency", f"""\t\t{ids['pkgdep']} /* OutlKit */ = {{
\t\t\tisa = XCSwiftPackageProductDependency;
\t\t\tpackage = {PKG} /* XCLocalSwiftPackageReference "../../../swift/OutlKit" */;
\t\t\tproductName = OutlKit;
\t\t}};
""")
open(pbx, "w").write(s)
print("added", name, ids['target'])
