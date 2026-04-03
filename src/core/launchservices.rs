use anyhow::{Result, bail};
use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};

use super::uti;

type OSStatus = i32;

#[allow(non_upper_case_globals)]
const kLSRolesAll: u32 = 0xFFFFFFFF;

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    static kUTTagClassFilenameExtension: CFStringRef;

    fn UTTypeCreatePreferredIdentifierForTag(
        in_tag_class: CFStringRef,
        in_tag: CFStringRef,
        in_conforming_to_uti: CFStringRef,
    ) -> CFStringRef;

    fn LSCopyDefaultRoleHandlerForContentType(content_type: CFStringRef, role: u32) -> CFStringRef;

    fn LSSetDefaultRoleHandlerForContentType(
        content_type: CFStringRef,
        role: u32,
        handler_bundle_id: CFStringRef,
    ) -> OSStatus;
}

/// Query the system for any UTI matching this extension, including dynamic UTIs.
/// Unlike `uti::uti_for_extension`, this does not reject `dyn.*` types, because
/// Launch Services can still resolve handlers for them.
fn any_uti_for_extension(ext: &str) -> Option<String> {
    let extension = CFString::new(ext);
    let uti_ref = unsafe {
        UTTypeCreatePreferredIdentifierForTag(
            kUTTagClassFilenameExtension,
            extension.as_concrete_TypeRef(),
            std::ptr::null(),
        )
    };

    if uti_ref.is_null() {
        return None;
    }

    let uti = unsafe { CFString::wrap_under_create_rule(uti_ref) }.to_string();
    if uti.is_empty() { None } else { Some(uti) }
}

/// Query the default application bundle ID for a file extension.
/// Returns `None` if no default is set.
///
/// Tries the well-known UTI first (from the hardcoded map / system lookup),
/// then falls back to any UTI the system assigns (including dynamic `dyn.*`
/// types) so that custom/non-standard extensions are still queryable.
pub fn query_default_bundle_id(ext: &str) -> Result<Option<String>> {
    let ext = ext.trim_start_matches('.');

    // Try well-known UTI first, fall back to any system UTI (including dyn.*)
    let uti_str = uti::uti_for_extension(ext)
        .ok()
        .or_else(|| any_uti_for_extension(ext));

    let uti_str = match uti_str {
        Some(u) => u,
        None => return Ok(None),
    };

    query_default_for_uti(&uti_str)
}

/// Query the default handler for a specific UTI string.
fn query_default_for_uti(uti_str: &str) -> Result<Option<String>> {
    let uti_cf = CFString::new(uti_str);
    let result = unsafe {
        LSCopyDefaultRoleHandlerForContentType(uti_cf.as_concrete_TypeRef(), kLSRolesAll)
    };

    if result.is_null() {
        return Ok(None);
    }

    let bundle_id = unsafe { CFString::wrap_under_create_rule(result) }.to_string();
    if bundle_id.is_empty() {
        Ok(None)
    } else {
        Ok(Some(bundle_id))
    }
}

/// Set the default application for a UTI.
pub fn set_default(bundle_id: &str, uti: &str) -> Result<()> {
    let uti_cf = CFString::new(uti);
    let bundle_cf = CFString::new(bundle_id);

    let status = unsafe {
        LSSetDefaultRoleHandlerForContentType(
            uti_cf.as_concrete_TypeRef(),
            kLSRolesAll,
            bundle_cf.as_concrete_TypeRef(),
        )
    };

    if status != 0 {
        bail!(
            "failed to set default handler (OSStatus {}). \
             The bundle ID '{}' may be invalid or the app may not be installed.",
            status,
            bundle_id
        );
    }

    Ok(())
}
