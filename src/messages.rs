//! Messages and resource strings copied verbatim from the original tool.

use crate::logger::{MessageType::Error as E, MessageType::Information as I, OutputMessage as M};

pub const TOOL_NAME: &str = "Vsix Publishing Tools";
pub const COPYRIGHT: &str = "Copyright (C) Christian Resma Helle. All rights reserved.";

// Command line parser strings (VsixCommandLine.dll).
pub const ARGUMENT_MISSING_VALUE: &str = "'{0}' requires a value.";
pub const INVALID_ARGUMENT: &str = "'{0}' is an invalid argument.";
pub const INVALID_COMMAND: &str = "The '{0}' command is invalid.";
pub const MISSING_COMMAND: &str = "A command is required to run the tool.";
pub const MISSING_REQUIRED_ARGUMENT: &str = "Missing required argument: '{0}'.";
pub const REQUIRED_ARGUMENT_FORMAT: &str = "Required. {0}";
pub const HELP_HELP_TEXT: &str = "Show the usage text";
pub const VERSION_HELP_TEXT: &str = "Display the version of this program.";

// Help texts.
pub const ARGUMENT_MARKETPLACE_URL: &str = "The VisualStudio Marketplace URL to use.";
pub const ARGUMENT_PERSONAL_ACCESS_TOKEN: &str = "The personal access token to use.";
pub const DELETE_EXTENSION_ARGUMENT_EXTENSION_NAME: &str = "The name of the extension to delete.";
pub const DELETE_EXTENSION_ARGUMENT_PUBLISHER_NAME: &str =
    "The publisher of the extension to delete.";
pub const DELETE_EXTENSION_HELP_TEXT: &str = "Delete an extension from the marketplace.";
pub const DELETE_PUBLISHER_ARGUMENT_PUBLISHER_NAME: &str = "The name of the publisher to delete.";
pub const DELETE_PUBLISHER_HELP_TEXT: &str = "Delete a publisher from the marketplace.";
pub const LOGIN_ARGUMENT_OVERWRITE: &str = "Specifies that any existing publisher should be overwritten with the new personal access token.";
pub const LOGIN_ARGUMENT_PUBLISHER_NAME: &str = "The name of the publisher to log-in.";
pub const LOGIN_HELP_TEXT: &str = "Add a publisher to the known publishers list.";
pub const LOGOUT_ARGUMENT_IGNORE_MISSING_PUBLISHER: &str =
    "Specifies that the tool should not error if the specified publisher is not already logged-in.";
pub const LOGOUT_ARGUMENT_PUBLISHER_NAME: &str = "The name of the publisher to log-out.";
pub const LOGOUT_HELP_TEXT: &str = "Remove a publisher from the known publishers list.";
pub const PUBLISHER_ARGUMENT_IGNORE_WARNINGS: &str = "List of warnings to ignore when publishing an extension (e.g. \"VSIXValidatorWarning01,VSIXValidatorWarning02\").";
pub const PUBLISHER_ARGUMENT_OIDC: &str = "Use trusted publishing with an OIDC token from the CI provider instead of a personal access token.";
pub const PUBLISHER_ARGUMENT_OUTPUT_FILE_PATH: &str = "The path to output the upload package to.";
pub const PUBLISHER_ARGUMENT_PAYLOAD: &str =
    "The path to the payload to publish or a URL to point users to to download your extension.";
pub const PUBLISHER_ARGUMENT_PERSONAL_ACCESS_TOKEN: &str = "The personal access token to use for publishing (if not supplied, the personal access token will be retrieved from the logged-in accounts).";
pub const PUBLISHER_ARGUMENT_PUBLISHER_MANIFEST: &str =
    "The path to the publishing manifest to use (a json file).";
pub const PUBLISHER_HELP_TEXT: &str = "Publishes an extension.";
pub const OIDC_TOKEN_RESPONSE_MISSING_TOKEN: &str = "The token response did not contain a token.";

pub const SUCCESS_EXTENSION_DELETION: &str = "Removed '{0}' from the marketplace.";
pub const SUCCESS_EXTENSION_UPLOAD: &str = "Uploaded '{0}' to the marketplace.";
pub const SUCCESS_PUBLISHER_CREATION: &str = "Added '{0}' as a publisher on the marketplace.";
pub const SUCCESS_PUBLISHER_DELETION: &str = "Removed '{0}' as a publisher on the marketplace.";
pub const SUCCESS_PUBLISHER_LOGIN: &str = "Publisher '{0}' is now logged-in.";
pub const SUCCESS_PUBLISHER_LOGOUT: &str = "Publisher '{0}' has been logged-out.";

pub const CANNOT_FIND_PAYLOAD_PATH: M = M::new(
    "Cannot find payload at specified path: '{0}'.",
    "VsixPub0001",
    E,
);
pub const CANNOT_FIND_PUBLISHER_MANIFEST_PATH: M = M::new(
    "Cannot find publisher manifest at specified path: '{0}'.",
    "VsixPub0002",
    E,
);
pub const ERROR_READING_PUBLISHER_MANIFEST: M = M::new(
    "Encountered an exception when reading the publisher manifest: {0}",
    "VsixPub0003",
    E,
);
pub const GENERIC_ERROR_PUBLISHING: M = M::new(
    "Encountered an exception when publishing the vsix from '{0}': {1}",
    "VsixPub0004",
    E,
);
pub const MANIFEST_MISSING_PUBLISHER: M = M::new(
    "The publish manifest from '{0}' is missing the publisher information.",
    "VsixPub0005",
    E,
);
pub const MANIFEST_INVALID_DATA: M = M::new(
    "The publish manifest from '{0}' contains some invalid entries: {1}.",
    "VsixPub0006",
    E,
);
pub const PUBLISHER_NOT_IN_VSCE: M = M::new(
    "Cannot locate the publisher '{0}'. Have you run the \"login\" command?",
    "VsixPub0007",
    E,
);
pub const ERROR_READING_PUBLISHER_DATABASE: M = M::new(
    "Encountered an exception when reading the publisher database file: {0}",
    "VsixPub0008",
    E,
);
pub const LONG_DESCRIPTION_NOT_FOUND: M = M::new(
    "The long description file could not be found at '{0}'.",
    "VsixPub0009",
    E,
);
pub const MISSING_PUBLISHER_NAME: M = M::new(
    "The '{0}' command requires a publisher name.",
    "VsixPub0010",
    E,
);
pub const MISSING_PAT_IN_LOGIN: M = M::new(
    "The 'login' command requires a personal access token.",
    "VsixPub0011",
    E,
);
pub const PUBLISHER_ALREADY_EXISTS: M = M::new(
    "The specified publisher is already logged-in: '{0}'.",
    "VsixPub0012",
    E,
);
pub const ERROR_WRITING_PUBLISHER_DATABASE: M = M::new(
    "Encountered an exception when updating the publisher database: {0}",
    "VsixPub0013",
    E,
);
pub const PUBLISHER_NOT_LOGGED_IN: M = M::new(
    "The specified publisher ({0}) is not logged-in.",
    "VsixPub0014",
    E,
);
pub const COULD_NOT_DELETE_PUBLISHER_DATABASE: M = M::new(
    "Encountered an exception while deleting the publisher database file: {0}",
    "VsixPub0015",
    E,
);
pub const PAYLOAD_NOT_VALID_VSIX: M = M::new(
    "The payload from '{0}' is not in a valid vsix format.",
    "VsixPub0016",
    E,
);
pub const MANIFEST_SHOULD_NOT_SET_IDENTITY: M = M::new(
    "The publish manifest from '{0}' cannot contain any identity information other than \"InternalName\" because the payload is a 'vsix' file.",
    "VsixPub0017",
    E,
);
pub const ERROR_PARSING_VSIX_MANIFEST: M = M::new(
    "Encountered an exception when parsing the vsixmanifest file: {0}",
    "VsixPub0018",
    E,
);
pub const PAYLOAD_REQUIRES_IDENTITY: M = M::new(
    "The payload from '{0}' is not a 'vsix' file so it requires additional identity information in the publish manifest.",
    "VsixPub0019",
    E,
);
pub const CANNOT_FIND_FILE_IN_VSIX: M = M::new(
    "Cannot find file '{0}' in the vsix from '{1}'.",
    "VsixPub0020",
    E,
);
pub const TOO_MANY_OR_TOO_FEW_CATEGORIES: M = M::new(
    "The publish manifest must contain between 1 and {0} categories.",
    "VsixPub0021",
    E,
);
pub const INVALID_PAYLOAD_ARGUMENT: M = M::new(
    "The \"payload\" argument '{0}' must be either a file (VSIX, EXE, or MSI) or a URL.",
    "VsixPub0022",
    E,
);
pub const TAG_TOO_LARGE: M = M::new(
    "The tag '{0}' is too large - the maximum length of a tag is {1}.",
    "VsixPub0023",
    E,
);
pub const DESCRIPTION_TOO_SHORT_OR_LONG: M = M::new(
    "The description for the extension must be set and it must be less than {0} characters in length.",
    "VsixPub0024",
    E,
);
pub const LANGUAGE_MUST_BE_SET: M = M::new(
    "The language must be set for the extension (e.g. \"en-US\").",
    "VsixPub0025",
    E,
);
pub const DISPLAY_NAME_MUST_BE_SET: M = M::new(
    "The display name must be set and must be less than {0} characters.",
    "VsixPub0026",
    E,
);
pub const REQUIRES_INSTALL_TARGET: M = M::new(
    "The extension must have at least one InstallTarget.",
    "VsixPub0027",
    E,
);
pub const SOURCE_REPO_INVALID: M = M::new(
    "The specified source control repo is not a valid URL: {0}",
    "VsixPub0028",
    E,
);
pub const ERROR_FROM_MARKETPLACE: M = M::new(
    "An error occurred while communicating with the marketplace: {0}",
    "VsixPub0029",
    E,
);
pub const PUBLISHER_NOT_FOUND_IN_MARKETPLACE: M = M::new(
    "The publisher '{0}' could not be found. You can create a publisher directly in the Marketplace: https://aka.ms/vsm-create-publisher",
    "VsixPub0030",
    E,
);
pub const COULD_NOT_AUTHENTICATE: M = M::new(
    "An error occurred while attempting to authenticate: {0}",
    "VsixPub0031",
    E,
);
pub const EXTENSION_NOT_FOUND: M = M::new(
    "Could not find extension '{0}' on the marketplace for publisher '{1}'.",
    "VsixPub0032",
    E,
);
pub const INVALID_INTERNAL_NAME: M = M::new(
    "The extension's internal name ({0}) is invalid in the publisher manifest. The 'internalName' value of 'identity' in the publisher manifest must be less than 63 characters, and can only contain 'A' through 'Z', 'a' through 'z', '0' through '9', and '-'. It must start with an alphanumeric character.",
    "VsixPub0033",
    E,
);
pub const INVALID_URI: M = M::new(
    "The URI '{0}' for the value '{1}' is invalid.",
    "VsixPub0034",
    E,
);
pub const AUTHENTICATION_TIMEOUT: M = M::new(
    "Could not authenticate the user: communication with the marketplace timed-out.",
    "VsixPub0035",
    E,
);
pub const PAYLOAD_FILENAME_INVALID: M = M::new(
    "The file name of the payload ({0}) contains some invalid characters.",
    "VsixPub0036",
    E,
);
pub const INFO_EXTENSION_DELETED: M = M::new(SUCCESS_EXTENSION_DELETION, "VsixPub0037", I);
pub const INFO_EXTENSION_UPLOADED: M = M::new(SUCCESS_EXTENSION_UPLOAD, "VsixPub0038", I);
pub const INFO_PUBLISHER_CREATED: M = M::new(SUCCESS_PUBLISHER_CREATION, "VsixPub0039", I);
pub const INFO_PUBLISHER_DELETED: M = M::new(SUCCESS_PUBLISHER_DELETION, "VsixPub0040", I);
pub const INFO_PUBLISHER_LOGGED_IN: M = M::new(SUCCESS_PUBLISHER_LOGIN, "VsixPub0041", I);
pub const INFO_PUBLISHER_LOGGED_OUT: M = M::new(SUCCESS_PUBLISHER_LOGOUT, "VsixPub0042", I);
pub const WARNINGS_FROM_MARKETPLACE: M = M::new(
    "The VSIX contains warnings preventing it from being uploaded to the Marketplace: {0}",
    "VsixPub0043",
    E,
);
pub const CONFLICTING_AUTHENTICATION_ARGUMENTS: M = M::new(
    "The '{0}' and '{1}' arguments cannot be used together.",
    "VsixPub0044",
    E,
);
pub const OIDC_PROVIDER_UNAVAILABLE: M = M::new(
    "Trusted publishing requires GitHub Actions OIDC environment variables '{0}' and '{1}'.",
    "VsixPub0045",
    E,
);
pub const OIDC_PROVIDER_TOKEN_REQUEST_FAILED: M = M::new(
    "Could not request an OIDC token from the CI provider: {0}",
    "VsixPub0046",
    E,
);
pub const OIDC_SESSION_TOKEN_EXCHANGE_FAILED: M = M::new(
    "Could not exchange the OIDC token for a Marketplace session token: {0}",
    "VsixPub0047",
    E,
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_map_to_codes() {
        assert_eq!(CANNOT_FIND_PAYLOAD_PATH.error_code(), 1);
        assert_eq!(OIDC_SESSION_TOKEN_EXCHANGE_FAILED.error_code(), 47);
        assert_eq!(
            PUBLISHER_NOT_IN_VSCE.output_text(&[&"me"]),
            "VSSDK: error VsixPub0007 : Cannot locate the publisher 'me'. Have you run the \"login\" command?"
        );
    }
}
