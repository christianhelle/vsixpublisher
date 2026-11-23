//! Category, PriceCategory and VisualStudioSku with the original name mappings.

macro_rules! category {
    ($($v:ident => $server:literal),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Category { $($v),* }

        impl Category {
            /// `CategoryExtensionMethods.ToServerFriendlyName`.
            pub fn server_name(self) -> &'static str {
                match self { $(Category::$v => $server),* }
            }
        }
    };
}

category! {
    AjaxTemplates => "ajax",
    AspNetControls => "aspnetcontrols",
    AspNetTemplates => "aspnet",
    Build => "build",
    Coding => "coding",
    ConnectedServices => "tools_connectedservices",
    Data => "data",
    DatabaseTemplates => "database",
    Documentation => "documentation",
    ExtensionSDK => "ExtensionSDK",
    FrameworkAndLibraries => "framework",
    LightSwitch => "lightswitchtools",
    LightSwitchControls => "lightswitchcontrols",
    LightSwitchTemplates => "lightswitchtemplates",
    Modeling => "modeling",
    OfficeTemplates => "office",
    Other => "othertools",
    OtherTemplates => "othertemplates",
    Performance => "performance",
    ProcessTemplates => "Process Templates",
    ProgrammingLanguages => "programmingLanguages",
    Reporting => "reporting",
    Scaffolding => "tools_scaffolding",
    Security => "security",
    Services => "tools_services",
    SetupAndDeployment => "setup",
    SharePoint => "SharePointtools",
    SharePointControls => "sharepointcontrols",
    SharePointTemplates => "SharePointtemplates",
    SilverlightControls => "silverlightcontrols",
    SourceControl => "sourcecontrol",
    StartPages => "startpages",
    TeamDevelopment => "teamdevelopment",
    Testing => "testing",
    Theme => "tools_theme",
    VisualStudioExtensions => "vsextensions",
    WcfTemplates => "wcf",
    Web => "web",
    WindowsFormsTemplates => "windowsforms",
    WindowsFormsControls => "windowsformscontrols",
    WindowsPhoneTemplates => "windowsphone",
    WorkflowTemplates => "workflow",
    WpfTemplates => "wpf",
    WpfControls => "wpfcontrols",
    XnaTemplates => "XNA Game Studio",
    ExtensionPacksTools => "extensionpacksvs",
}

impl Category {
    /// `CategoryConverter.ReadJson`: lower-cased, spaces removed.
    pub fn parse(text: &str) -> Result<Self, String> {
        use Category::*;
        let key = text.to_lowercase().replace(' ', "");
        Ok(match key.as_str() {
            "ajax" | "ajaxtemplates" => AjaxTemplates,
            "build" => Build,
            "coding" => Coding,
            "connectedservices" => ConnectedServices,
            "data" => Data,
            "databasetemplates" | "database" => DatabaseTemplates,
            "documentation" => Documentation,
            "extensionsdk" => ExtensionSDK,
            "frameworkandlibraries" | "framework&libraries" => FrameworkAndLibraries,
            "lightswitch" | "lightswitchtools" => LightSwitch,
            "lightswitchcontrols" => LightSwitchControls,
            "lightswitchtemplates" => LightSwitchTemplates,
            "modeling" => Modeling,
            "office" | "officetemplates" => OfficeTemplates,
            "other" => Other,
            "othertemplates" => OtherTemplates,
            "performance" => Performance,
            "processtemplates" => ProcessTemplates,
            "programminglanguages" => ProgrammingLanguages,
            "reporting" => Reporting,
            "scaffolding" => Scaffolding,
            "security" => Security,
            "services" => Services,
            "setup&deployment" | "setupanddeployment" => SetupAndDeployment,
            "sharepointtools" | "sharepoint" => SharePoint,
            "sharepointcontrols" => SharePointControls,
            "sharepointtemplates" => SharePointTemplates,
            "silverlightcontrols" => SilverlightControls,
            "sourcecontrol" => SourceControl,
            "startpages" => StartPages,
            "teamdevelopment" => TeamDevelopment,
            "testing" => Testing,
            "theme" => Theme,
            "visualstudioextensions" => VisualStudioExtensions,
            "wcftemplates" | "wcf" => WcfTemplates,
            "web" => Web,
            "windowsformstemplates" => WindowsFormsTemplates,
            "windowsformscontrols" => WindowsFormsControls,
            "workflowtemplates" | "workflow" => WorkflowTemplates,
            "wpftemplates" => WpfTemplates,
            "wpfcontrols" => WpfControls,
            "xnatemplates" | "xna" => XnaTemplates,
            _ => return Err(format!("Encountered an unsupported category: {text}.")),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PriceCategory {
    #[default]
    Free,
    Paid,
    Trial,
}

impl PriceCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            PriceCategory::Free => "Free",
            PriceCategory::Paid => "Paid",
            PriceCategory::Trial => "Trial",
        }
    }

    /// Newtonsoft enum semantics: case-insensitive name or underlying integer.
    pub fn parse(text: &str) -> Result<Self, String> {
        match text.trim().to_lowercase().as_str() {
            "free" | "0" => Ok(PriceCategory::Free),
            "paid" | "1" => Ok(PriceCategory::Paid),
            "trial" | "2" => Ok(PriceCategory::Trial),
            _ => Err(format!("Requested value '{text}' was not found.")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VisualStudioSku {
    Community,
    Premium,
    Enterprise,
    ExpressAll,
    IntegratedShell,
    Pro,
    TestProfessional,
    Ultimate,
    VBExpress,
    VCExpress,
    VCSExpress,
    VPDExpress,
    VSLS,
    VSWinExpress,
    VSWinDesktopExpress,
    WebExpress,
}

impl VisualStudioSku {
    /// `Utilities.GetVisualStudioSkuForInputString`.
    pub fn parse(name: &str) -> Result<Self, String> {
        use VisualStudioSku::*;
        Ok(match name.to_lowercase().as_str() {
            "integrated" | "microsoft.visualstudio.integratedshell" => IntegratedShell,
            "community" | "microsoft.visualstudio.community" | "microsoft.visualstudio.communityx86" => Community,
            "microsoft.visualstudio.professionalx86" | "microsoft.visualstudio.professional" | "pro"
            | "professional" | "microsoft.visualstudio.pro" => Pro,
            "premium" | "microsoft.visualstudio.premium" => Premium,
            "enterprise" | "microsoft.visualstudio.enterprise" | "microsoft.visualstudio.enterprisex86" => Enterprise,
            "winexpress" | "microsoft.visualstudio.vswinexpress" | "win" | "vswinexpress" => VSWinExpress,
            "vwdexpress" | "webexpress" | "vwd" | "microsoft.visualstudio.vwdexpress"
            | "microsoft.visualstudio.webexpress" => WebExpress,
            "windesktop" | "windesktopexpress" | "vswindesktopexpress"
            | "microsoft.visualstudio.vswindesktopexpress" => VSWinDesktopExpress,
            "express_all" | "microsoft.visualstudio.express_all" => ExpressAll,
            "microsoft.visualstudio.test" | "testprofessional" | "microsoft.visualstudio.testprofessional" => {
                TestProfessional
            }
            "vbexpress" | "microsoft.visualstudio.vbexpress" => VBExpress,
            "vcexpress" | "microsoft.visualstudio.vcexpress" => VCExpress,
            "vcsexpress" | "microsoft.visualstudio.vcsexpress" => VCSExpress,
            "vpdexpress" | "microsoft.visualstudio.vpdexpress" => VPDExpress,
            "microsoft.visualstudio.vsls" | "vsls" => VSLS,
            "ultimate" | "microsoft.visualstudio.ultimate" => Ultimate,
            _ => return Err(format!("Could not find matching sku for {name}.")),
        })
    }

    /// `VisualStudioSkuExtensions.ToFullyQualifiedName`.
    pub fn fully_qualified_name(self) -> String {
        use VisualStudioSku::*;
        let n = match self {
            Community => "Community",
            Premium => "Premium",
            Enterprise => "Enterprise",
            IntegratedShell => "IntegratedShell",
            Pro => "Pro",
            Ultimate => "Ultimate",
            VBExpress => "VBExpress",
            VCExpress => "VCExpress",
            VCSExpress => "VCSExpress",
            VPDExpress => "VPDExpress",
            VSLS => "VSLS",
            VSWinExpress => "VSWinExpress",
            VSWinDesktopExpress => "VSWinDesktopExpress",
            ExpressAll => "Express_All",
            WebExpress => "VWDExpress",
            TestProfessional => "Test",
        };
        format!("Microsoft.VisualStudio.{n}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_aliases() {
        assert_eq!(Category::parse("Coding").unwrap(), Category::Coding);
        assert_eq!(Category::parse("Setup & Deployment").unwrap(), Category::SetupAndDeployment);
        assert_eq!(Category::parse("Framework and Libraries").unwrap(), Category::FrameworkAndLibraries);
        assert_eq!(Category::parse("other").unwrap().server_name(), "othertools");
        assert_eq!(Category::parse("services").unwrap().server_name(), "tools_services");
        assert_eq!(
            Category::parse("bogus").unwrap_err(),
            "Encountered an unsupported category: bogus."
        );
    }

    #[test]
    fn price_category() {
        assert_eq!(PriceCategory::parse("paid").unwrap(), PriceCategory::Paid);
        assert_eq!(PriceCategory::parse("2").unwrap(), PriceCategory::Trial);
        assert!(PriceCategory::parse("cheap").is_err());
        assert_eq!(PriceCategory::default().as_str(), "Free");
    }

    #[test]
    fn sku_mapping() {
        assert_eq!(VisualStudioSku::parse("Microsoft.VisualStudio.Pro").unwrap(), VisualStudioSku::Pro);
        assert_eq!(VisualStudioSku::parse("community").unwrap().fully_qualified_name(), "Microsoft.VisualStudio.Community");
        assert_eq!(VisualStudioSku::parse("webexpress").unwrap().fully_qualified_name(), "Microsoft.VisualStudio.VWDExpress");
        assert_eq!(VisualStudioSku::parse("testprofessional").unwrap().fully_qualified_name(), "Microsoft.VisualStudio.Test");
        assert!(VisualStudioSku::parse("Microsoft.VisualStudio.Foo").is_err());
    }
}
