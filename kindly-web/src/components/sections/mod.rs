// Section components (Organism layer - Hero, Features, Pricing, Comparison, Security, Testimonials, CTA, Footer)
mod hero;
mod features;
mod pricing;
mod comparison;
mod security;
mod testimonials;
mod cta;
mod footer;

// New sections for kindly_dedup landing page
mod performance;
mod demo;
mod api;
mod faq;

// Legal pages
mod terms;

// Allow unused imports - part of design system public API
#[allow(unused_imports)]
pub use hero::Hero;
#[allow(unused_imports)]
pub use features::Features;
#[allow(unused_imports)]
pub use pricing::Pricing;
#[allow(unused_imports)]
pub use comparison::Comparison;
#[allow(unused_imports)]
pub use security::Security;
#[allow(unused_imports)]
pub use testimonials::Testimonials;
#[allow(unused_imports)]
pub use cta::CallToAction;
#[allow(unused_imports)]
pub use footer::Footer;

// New section exports
#[allow(unused_imports)]
pub use performance::Performance;
#[allow(unused_imports)]
pub use demo::Demo;
#[allow(unused_imports)]
pub use api::ApiPreview;
#[allow(unused_imports)]
pub use faq::FAQ;

// Legal page exports
#[allow(unused_imports)]
pub use terms::Terms;
