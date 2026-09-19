use anyhow::{Result, bail};
use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};

use super::uti;

type OSStatus = i32;

#[allow(non_upper_case_globals)]
const kLSRolesAll: u32 = 0xFFFFFFFF;

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    fn LSCopyDefaultRoleHandlerForContentType(content_type: CFStringRef, role: u32) -> CFStringRef;

    fn LSSetDefaultRoleHandlerForContentType(
        content_type: CFStringRef,
        role: u32,
        handler_bundle_id: CFStringRef,
    ) -> OSStatus;

    fn LSCopyDefaultHandlerForURLScheme(scheme: CFStringRef) -> CFStringRef;

    fn LSSetDefaultHandlerForURLScheme(
        scheme: CFStringRef,
        handler_bundle_id: CFStringRef,
    ) -> OSStatus;
}

/// Query the default application bundle ID for a file extension.
/// Returns `None` if no default is set.
///
/// Reads through the same UTI the writer targets, so a query can never report
/// a handler for a type that setting the default would not touch.
pub fn query_default_bundle_id(ext: &str) -> Result<Option<String>> {
    let ext = ext.trim_start_matches('.');

    let uti_str = match uti::uti_for_extension(ext) {
        Ok(u) => u,
        Err(_) => return Ok(None),
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

/// Query the default handler bundle ID for a URL scheme (e.g. "http").
/// Returns `None` if no default is set.
pub fn query_default_scheme_handler(scheme: &str) -> Result<Option<String>> {
    let scheme_cf = CFString::new(scheme);
    let result = unsafe { LSCopyDefaultHandlerForURLScheme(scheme_cf.as_concrete_TypeRef()) };

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

/// Set the default handler for a URL scheme.
pub fn set_default_scheme_handler(bundle_id: &str, scheme: &str) -> Result<()> {
    let scheme_cf = CFString::new(scheme);
    let bundle_cf = CFString::new(bundle_id);

    let status = unsafe {
        LSSetDefaultHandlerForURLScheme(
            scheme_cf.as_concrete_TypeRef(),
            bundle_cf.as_concrete_TypeRef(),
        )
    };

    if status != 0 {
        bail!(
            "failed to set handler for {}:// (OSStatus {}). \
             The bundle ID '{}' may be invalid or the app may not declare the scheme.",
            scheme,
            status,
            bundle_id
        );
    }

    Ok(())
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
