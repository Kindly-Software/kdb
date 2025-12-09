# Component Documentation

**kindly-web Component System** - Comprehensive reference for all 33 UI components

Version: 1.0
Date: 2025-10-18
Architecture: Atomic Design (Atoms → Molecules → Organisms)

---

## Table of Contents

1. [Overview](#overview)
2. [Atomic Components (Tier 1)](#tier-1-atoms-11-components)
3. [Molecular Components (Tier 2)](#tier-2-molecules-12-components)
4. [Organism Components (Tier 3)](#tier-3-organisms-10-components)
5. [Usage Patterns](#usage-patterns)
6. [Styling System](#styling-system)
7. [Accessibility](#accessibility)

---

## Overview

### Component Philosophy

All components follow **Atomic Design** principles:
- **Atoms**: Primitive building blocks (Button, Card, Icon, Text)
- **Molecules**: Composed components with specific functionality (Navbar, PriceCard)
- **Organisms**: Complex sections combining molecules and atoms (Hero, Features, Pricing)

**Key Principles**:
- ✅ **100% Leptos**: Built with Leptos 0.7 reactive framework
- ✅ **Type-Safe Props**: Compile-time validation via Rust's type system
- ✅ **Accessibility First**: WCAG 2.1 AA compliance (keyboard navigation, ARIA labels)
- ✅ **Performance**: Minimal re-renders via fine-grained reactivity
- ✅ **Byzantine Purple**: Consistent design system (87 tokens)

### Import Pattern

```rust
use leptos::prelude::*;
use kindly_web::components::*;

#[component]
pub fn MyPage() -> impl IntoView {
    view! {
        <Button variant="primary" size="large">
            "Get Started"
        </Button>
    }
}
```

---

## Tier 1: Atoms (11 Components)

### 1. Button

**Purpose**: Primary, secondary, and ghost buttons for user actions

**Props**:

```rust
#[component]
pub fn Button(
    /// Button variant: "primary" | "secondary" | "ghost" | "danger"
    #[prop(default = "primary")]
    variant: &'static str,

    /// Button size: "small" | "medium" | "large"
    #[prop(default = "medium")]
    size: &'static str,

    /// Disabled state
    #[prop(default = false)]
    disabled: bool,

    /// Click handler
    #[prop(optional)]
    on_click: Option<Box<dyn Fn(ev::MouseEvent)>>,

    /// Child content
    children: Children,
) -> impl IntoView
```

**Variants**:
- **primary**: Byzantine Purple (#4B0082), white text, full background
- **secondary**: Transparent, purple border, purple text
- **ghost**: Transparent, no border, purple text (hover: light purple background)
- **danger**: Red (#DC2626), white text, for destructive actions

**Sizes**:
- **small**: 32px height, 12px padding, 14px font
- **medium**: 40px height, 16px padding, 16px font
- **large**: 48px height, 20px padding, 18px font

**Example**:

```rust
view! {
    <Button variant="primary" size="large" on_click=move |_| {
        // Handle click
    }>
        "Get Started"
    </Button>

    <Button variant="secondary" size="medium">
        "Learn More"
    </Button>

    <Button variant="ghost" size="small" disabled=true>
        "Disabled"
    </Button>
}
```

**Accessibility**:
- ✅ Keyboard focusable (`tabindex="0"`)
- ✅ Enter/Space key activation
- ✅ Disabled state (`aria-disabled="true"`)
- ✅ 4.5:1 color contrast ratio (WCAG AA)

---

### 2. Card

**Purpose**: Content container with elevation, padding, and border radius

**Props**:

```rust
#[component]
pub fn Card(
    /// Card variant: "default" | "outlined" | "elevated"
    #[prop(default = "default")]
    variant: &'static str,

    /// Padding: "none" | "small" | "medium" | "large"
    #[prop(default = "medium")]
    padding: &'static str,

    /// Shadow elevation (0-4)
    #[prop(default = 1)]
    shadow: u8,

    /// Optional CSS class
    #[prop(optional)]
    class: &'static str,

    /// Child content
    children: Children,
) -> impl IntoView
```

**Variants**:
- **default**: White background, subtle border, slight shadow
- **outlined**: Transparent background, prominent border, no shadow
- **elevated**: White background, no border, prominent shadow

**Example**:

```rust
view! {
    <Card variant="elevated" padding="large" shadow=2>
        <h3>"Feature Title"</h3>
        <p>"Feature description goes here."</p>
    </Card>
}
```

---

### 3. Icon

**Purpose**: SVG icon display with size and color variants

**Props**:

```rust
#[component]
pub fn Icon(
    /// Icon name: "menu" | "close" | "chevron-down" | "check" | "arrow-right" | etc.
    name: &'static str,

    /// Icon size: "small" (16px) | "medium" (24px) | "large" (32px)
    #[prop(default = "medium")]
    size: &'static str,

    /// Color: "primary" | "secondary" | "white" | "black" | CSS color
    #[prop(default = "primary")]
    color: &'static str,
) -> impl IntoView
```

**Available Icons** (20 total):
- **Navigation**: menu, close, chevron-down, chevron-up, chevron-left, chevron-right, arrow-right
- **Actions**: check, checkmark, plus, minus, search, filter, settings
- **Status**: info, warning, error, success
- **Social**: github, twitter, linkedin

**Example**:

```rust
view! {
    <Icon name="check" size="small" color="success" />
    <Icon name="menu" size="medium" color="white" />
    <Icon name="arrow-right" size="large" color="primary" />
}
```

**SVG Implementation**:
- ✅ Inline SVG (no external requests)
- ✅ Responsive sizing via CSS
- ✅ Color inheritance via `currentColor`

---

### 4. Text

**Purpose**: Typography variants (headings, body, captions)

**Props**:

```rust
#[component]
pub fn Text(
    /// Text variant: "h1" | "h2" | "h3" | "h4" | "body" | "caption" | "label"
    #[prop(default = "body")]
    variant: &'static str,

    /// Font size override (CSS units)
    #[prop(optional)]
    size: Option<&'static str>,

    /// Font weight: "normal" | "medium" | "semibold" | "bold"
    #[prop(optional)]
    weight: Option<&'static str>,

    /// Text color: "primary" | "secondary" | "muted" | CSS color
    #[prop(default = "primary")]
    color: &'static str,

    /// Text alignment: "left" | "center" | "right"
    #[prop(default = "left")]
    align: &'static str,

    /// Child content
    children: Children,
) -> impl IntoView
```

**Typography Scale**:
- **h1**: 48px, bold, 1.2 line-height
- **h2**: 36px, semibold, 1.3 line-height
- **h3**: 28px, semibold, 1.4 line-height
- **h4**: 20px, medium, 1.5 line-height
- **body**: 16px, normal, 1.6 line-height
- **caption**: 14px, normal, 1.5 line-height
- **label**: 12px, medium, 1.4 line-height

**Example**:

```rust
view! {
    <Text variant="h1" color="primary">"Welcome to kindly.ai"</Text>
    <Text variant="body" color="muted">"Build faster with Pure Rust WASM"</Text>
    <Text variant="caption" color="secondary">"Powered by computational capsules"</Text>
}
```

---

### 5. Link

**Purpose**: Navigation links (internal routes, external URLs)

**Props**:

```rust
#[component]
pub fn Link(
    /// Link href (internal route or external URL)
    href: &'static str,

    /// Link variant: "default" | "button" | "underline" | "nav"
    #[prop(default = "default")]
    variant: &'static str,

    /// External link (opens in new tab)
    #[prop(default = false)]
    external: bool,

    /// Optional CSS class
    #[prop(optional)]
    class: &'static str,

    /// Child content
    children: Children,
) -> impl IntoView
```

**Variants**:
- **default**: Purple text, underline on hover
- **button**: Button-like appearance (inherits Button styling)
- **underline**: Always underlined, purple text
- **nav**: Navbar link (no underline, bold on active)

**Example**:

```rust
view! {
    // Internal route (client-side navigation)
    <Link href="/pricing" variant="default">
        "View Pricing"
    </Link>

    // External link (opens in new tab)
    <Link href="https://github.com/kindly-ai" variant="button" external=true>
        "View on GitHub"
    </Link>
}
```

**Accessibility**:
- ✅ External links have `rel="noopener noreferrer"` (security)
- ✅ Keyboard navigable
- ✅ `aria-label` for icon-only links

---

### 6. Badge

**Purpose**: Status indicators, labels, tags

**Props**:

```rust
#[component]
pub fn Badge(
    /// Badge variant: "default" | "success" | "warning" | "danger" | "info"
    #[prop(default = "default")]
    variant: &'static str,

    /// Badge size: "small" | "medium" | "large"
    #[prop(default = "small")]
    size: &'static str,

    /// Child content
    children: Children,
) -> impl IntoView
```

**Variants** (with colors):
- **default**: Gray background (#E5E7EB), dark text
- **success**: Green background (#10B981), white text
- **warning**: Yellow background (#F59E0B), dark text
- **danger**: Red background (#EF4444), white text
- **info**: Blue background (#3B82F6), white text

**Example**:

```rust
view! {
    <Badge variant="success" size="small">"New"</Badge>
    <Badge variant="warning" size="medium">"Beta"</Badge>
    <Badge variant="info" size="large">"Popular"</Badge>
}
```

---

### 7. Input

**Purpose**: Form input fields (text, email, password, search)

**Props**:

```rust
#[component]
pub fn Input(
    /// Input type: "text" | "email" | "password" | "search" | "number"
    #[prop(default = "text")]
    type_: &'static str,

    /// Placeholder text
    #[prop(optional)]
    placeholder: Option<&'static str>,

    /// Current value
    #[prop(optional)]
    value: Option<String>,

    /// Input handler
    #[prop(optional)]
    on_input: Option<Box<dyn Fn(String)>>,

    /// Disabled state
    #[prop(default = false)]
    disabled: bool,

    /// Error state
    #[prop(default = false)]
    error: bool,

    /// Optional label
    #[prop(optional)]
    label: Option<&'static str>,
) -> impl IntoView
```

**Example**:

```rust
let (email, set_email) = signal(String::new());

view! {
    <Input
        type_="email"
        placeholder="Enter your email"
        value=email.get()
        on_input=move |val| set_email.set(val)
        label="Email Address"
    />
}
```

**Validation**:
- ✅ HTML5 validation (email, URL patterns)
- ✅ Error state styling (red border)
- ✅ Disabled state (grayed out)

---

### 8. Checkbox

**Purpose**: Boolean selection (feature comparison, form consent)

**Props**:

```rust
#[component]
pub fn Checkbox(
    /// Checked state
    #[prop(default = false)]
    checked: bool,

    /// Change handler
    #[prop(optional)]
    on_change: Option<Box<dyn Fn(bool)>>,

    /// Disabled state
    #[prop(default = false)]
    disabled: bool,

    /// Optional label
    #[prop(optional)]
    label: Option<&'static str>,
) -> impl IntoView
```

**Example**:

```rust
let (accepted, set_accepted) = signal(false);

view! {
    <Checkbox
        checked=accepted.get()
        on_change=move |val| set_accepted.set(val)
        label="I accept the terms and conditions"
    />
}
```

---

### 9. Divider

**Purpose**: Visual separator between sections

**Props**:

```rust
#[component]
pub fn Divider(
    /// Orientation: "horizontal" | "vertical"
    #[prop(default = "horizontal")]
    orientation: &'static str,

    /// Spacing (margin): "small" | "medium" | "large"
    #[prop(default = "medium")]
    spacing: &'static str,
) -> impl IntoView
```

**Example**:

```rust
view! {
    <Section />
    <Divider orientation="horizontal" spacing="large" />
    <Section />
}
```

---

### 10. Spinner

**Purpose**: Loading indicator for async operations

**Props**:

```rust
#[component]
pub fn Spinner(
    /// Spinner size: "small" (16px) | "medium" (24px) | "large" (32px)
    #[prop(default = "medium")]
    size: &'static str,

    /// Spinner color: "primary" | "white" | CSS color
    #[prop(default = "primary")]
    color: &'static str,
) -> impl IntoView
```

**Example**:

```rust
view! {
    <Show when=move || loading.get() fallback=|| view! { <Content /> }>
        <Spinner size="large" color="primary" />
    </Show>
}
```

---

### 11. Avatar

**Purpose**: User avatar (image or initials)

**Props**:

```rust
#[component]
pub fn Avatar(
    /// Avatar size: "small" (32px) | "medium" (48px) | "large" (64px)
    #[prop(default = "medium")]
    size: &'static str,

    /// Image source
    #[prop(optional)]
    src: Option<&'static str>,

    /// Alt text
    #[prop(optional)]
    alt: Option<&'static str>,

    /// Fallback initials
    #[prop(optional)]
    initials: Option<&'static str>,
) -> impl IntoView
```

**Example**:

```rust
view! {
    <Avatar src="/avatar.jpg" alt="User Name" size="large" />
    <Avatar initials="JD" size="medium" />
}
```

---

## Tier 2: Molecules (12 Components)

### 1. Navbar

**Purpose**: Site navigation header with logo, links, and CTA

**Props**:

```rust
#[component]
pub fn Navbar() -> impl IntoView
```

**Structure**:
- **Logo**: kindly.ai branding (left-aligned)
- **Navigation Links**: Home, Features, Pricing, About, Docs
- **CTA Button**: "Get Started" (right-aligned)
- **Mobile Menu**: Hamburger icon (< 768px)

**Example**:

```rust
view! {
    <Navbar />
    <main>
        // Page content
    </main>
}
```

**Responsive Behavior**:
- **Desktop (>768px)**: Horizontal navigation with inline links
- **Mobile (<768px)**: Hamburger menu with slide-out drawer

---

### 2. Footer

**Purpose**: Site footer with links, copyright, social icons

**Structure**:
- **Columns**:
  - Product: Features, Pricing, Changelog, Docs
  - Company: About, Blog, Careers, Contact
  - Legal: Privacy, Terms, Security
- **Social Icons**: GitHub, Twitter, LinkedIn
- **Copyright**: "© 2025 kindly.ai. All rights reserved."

**Example**:

```rust
view! {
    <Footer />
}
```

---

### 3. PriceCard

**Purpose**: Pricing tier display with features, price, and CTA

**Props**:

```rust
#[component]
pub fn PriceCard(
    /// Pricing tier name
    tier: &'static str,

    /// Price (e.g., "$29")
    price: &'static str,

    /// Billing period (e.g., "/month")
    period: &'static str,

    /// Feature list
    features: Vec<&'static str>,

    /// CTA button text
    cta: &'static str,

    /// Highlighted tier (most popular)
    #[prop(default = false)]
    highlighted: bool,
) -> impl IntoView
```

**Example**:

```rust
view! {
    <PriceCard
        tier="Pro"
        price="$99"
        period="/month"
        features=vec![
            "10,000 requests/month",
            "Priority support",
            "Advanced analytics",
            "Custom integrations",
        ]
        cta="Get Started"
        highlighted=true
    />
}
```

---

### 4. FeatureCard

**Purpose**: Feature showcase with icon, title, description

**Props**:

```rust
#[component]
pub fn FeatureCard(
    /// Icon name
    icon: &'static str,

    /// Feature title
    title: &'static str,

    /// Feature description
    description: &'static str,
) -> impl IntoView
```

**Example**:

```rust
view! {
    <FeatureCard
        icon="zap"
        title="Lightning Fast"
        description="Sub-10ns state reads with computational capsules"
    />
}
```

---

### 5. TestimonialCard

**Purpose**: Customer quote with avatar, name, role

**Props**:

```rust
#[component]
pub fn TestimonialCard(
    /// Customer quote
    quote: &'static str,

    /// Customer name
    name: &'static str,

    /// Customer role/company
    role: &'static str,

    /// Avatar image
    #[prop(optional)]
    avatar_src: Option<&'static str>,
) -> impl IntoView
```

**Example**:

```rust
view! {
    <TestimonialCard
        quote="kindly.ai cut our API costs by 40% while improving performance."
        name="Jane Doe"
        role="CTO, Example Corp"
        avatar_src="/avatars/jane.jpg"
    />
}
```

---

### 6. NewsletterForm

**Purpose**: Email subscription form with validation

**Structure**:
- Email input field
- Subscribe button
- Privacy disclaimer

**Example**:

```rust
view! {
    <NewsletterForm />
}
```

**Validation**:
- ✅ HTML5 email validation
- ✅ Success toast on submission
- ✅ Error handling for invalid emails

---

### 7. SearchBar

**Purpose**: Search input with icon and submit button

**Props**:

```rust
#[component]
pub fn SearchBar(
    /// Placeholder text
    #[prop(default = "Search...")]
    placeholder: &'static str,

    /// Search handler
    on_search: Box<dyn Fn(String)>,
) -> impl IntoView
```

**Example**:

```rust
view! {
    <SearchBar
        placeholder="Search documentation..."
        on_search=move |query| {
            // Handle search
        }
    />
}
```

---

### 8. Breadcrumb

**Purpose**: Navigation breadcrumb trail

**Props**:

```rust
#[component]
pub fn Breadcrumb(
    /// Breadcrumb items (label, href)
    items: Vec<(&'static str, &'static str)>,
) -> impl IntoView
```

**Example**:

```rust
view! {
    <Breadcrumb items=vec![
        ("Home", "/"),
        ("Docs", "/docs"),
        ("Components", "/docs/components"),
    ] />
}
```

---

### 9. Alert

**Purpose**: Notification banner (success, error, warning, info)

**Props**:

```rust
#[component]
pub fn Alert(
    /// Alert variant: "success" | "error" | "warning" | "info"
    variant: &'static str,

    /// Alert message
    message: &'static str,

    /// Dismissible
    #[prop(default = true)]
    dismissible: bool,
) -> impl IntoView
```

**Example**:

```rust
view! {
    <Alert variant="success" message="Profile updated successfully" />
    <Alert variant="error" message="Failed to save changes" dismissible=false />
}
```

---

### 10. Modal

**Purpose**: Dialog overlay for forms, confirmations

**Props**:

```rust
#[component]
pub fn Modal(
    /// Modal open state
    open: bool,

    /// Close handler
    on_close: Box<dyn Fn()>,

    /// Modal title
    #[prop(optional)]
    title: Option<&'static str>,

    /// Child content
    children: Children,
) -> impl IntoView
```

**Example**:

```rust
let (modal_open, set_modal_open) = signal(false);

view! {
    <Button on_click=move |_| set_modal_open.set(true)>
        "Open Modal"
    </Button>

    <Modal
        open=modal_open.get()
        on_close=move || set_modal_open.set(false)
        title="Confirm Action"
    >
        <p>"Are you sure you want to proceed?"</p>
        <Button on_click=move |_| {
            // Confirm action
            set_modal_open.set(false);
        }>
            "Confirm"
        </Button>
    </Modal>
}
```

---

### 11. Tooltip

**Purpose**: Contextual help on hover

**Props**:

```rust
#[component]
pub fn Tooltip(
    /// Tooltip content
    content: &'static str,

    /// Trigger element
    children: Children,
) -> impl IntoView
```

**Example**:

```rust
view! {
    <Tooltip content="Click to copy">
        <Button variant="ghost" size="small">
            <Icon name="copy" />
        </Button>
    </Tooltip>
}
```

---

### 12. Tabs

**Purpose**: Tabbed interface for content organization

**Props**:

```rust
#[component]
pub fn Tabs(
    /// Tab items (label, content)
    items: Vec<(&'static str, View)>,

    /// Default active tab index
    #[prop(default = 0)]
    default_active: usize,
) -> impl IntoView
```

**Example**:

```rust
view! {
    <Tabs items=vec![
        ("Overview", view! { <Overview /> }),
        ("Pricing", view! { <Pricing /> }),
        ("FAQ", view! { <FAQ /> }),
    ] />
}
```

---

## Tier 3: Organisms (10 Components)

### 1. Hero

**Purpose**: Landing page hero section with headline, CTA, and image

**Props**:

```rust
#[component]
pub fn Hero(
    /// Hero headline
    headline: &'static str,

    /// Hero subheadline
    subheadline: &'static str,

    /// Primary CTA button text
    cta_primary: &'static str,

    /// Secondary CTA button text
    #[prop(optional)]
    cta_secondary: Option<&'static str>,

    /// Hero image source
    #[prop(optional)]
    image_src: Option<&'static str>,
) -> impl IntoView
```

**Example**:

```rust
view! {
    <Hero
        headline="Build Faster with Pure Rust WASM"
        subheadline="Computational capsule architecture for 10× performance"
        cta_primary="Get Started"
        cta_secondary="View Demo"
        image_src="/hero-image.webp"
    />
}
```

---

### 2. Features

**Purpose**: Feature grid with icons, titles, descriptions

**Props**:

```rust
#[component]
pub fn Features(
    /// Section headline
    headline: &'static str,

    /// Feature items
    features: Vec<Feature>,
) -> impl IntoView

pub struct Feature {
    pub icon: &'static str,
    pub title: &'static str,
    pub description: &'static str,
}
```

**Example**:

```rust
view! {
    <Features
        headline="Why Choose kindly.ai"
        features=vec![
            Feature {
                icon: "zap",
                title: "Lightning Fast",
                description: "Sub-10ns state reads with atomic capsules",
            },
            Feature {
                icon: "shield",
                title: "Compile-Time Safe",
                description: "Zero runtime errors with verification macros",
            },
            Feature {
                icon: "box",
                title: "Tiny Bundle",
                description: "180KB gzipped WASM, 52% under budget",
            },
        ]
    />
}
```

---

### 3. Pricing

**Purpose**: Pricing table with comparison

**Example**:

```rust
view! {
    <Pricing />
}
```

**Structure**:
- 3 pricing tiers (Starter, Pro, Enterprise)
- Feature comparison
- CTA buttons for each tier

---

### 4. Comparison

**Purpose**: Feature comparison table across pricing tiers

**Example**:

```rust
view! {
    <Comparison />
}
```

---

### 5. Security

**Purpose**: Security features and compliance badges

**Example**:

```rust
view! {
    <Security />
}
```

---

### 6. Testimonials

**Purpose**: Customer testimonials carousel

**Example**:

```rust
view! {
    <Testimonials />
}
```

---

### 7. CallToAction

**Purpose**: Call-to-action banner with headline, CTA

**Props**:

```rust
#[component]
pub fn CallToAction(
    /// CTA headline
    headline: &'static str,

    /// CTA subheadline
    #[prop(optional)]
    subheadline: Option<&'static str>,

    /// CTA button text
    cta: &'static str,
) -> impl IntoView
```

**Example**:

```rust
view! {
    <CallToAction
        headline="Ready to Build?"
        subheadline="Start your free trial today"
        cta="Get Started"
    />
}
```

---

### 8. FAQ

**Purpose**: Frequently asked questions (accordion-style)

**Example**:

```rust
view! {
    <FAQ />
}
```

---

### 9. Team

**Purpose**: Team member grid with photos, names, roles

**Example**:

```rust
view! {
    <Team />
}
```

---

### 10. Contact

**Purpose**: Contact form with validation

**Example**:

```rust
view! {
    <Contact />
}
```

---

## Usage Patterns

### Pattern 1: Composing a Page

```rust
use leptos::prelude::*;
use kindly_web::components::*;

#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <>
            <Hero
                headline="Build Faster with Pure Rust WASM"
                subheadline="Computational capsule architecture for 10× performance"
                cta_primary="Get Started"
                cta_secondary="View Demo"
                image_src="/hero-image.webp"
            />
            <Features
                headline="Why Choose kindly.ai"
                features=vec![
                    Feature {
                        icon: "zap",
                        title: "Lightning Fast",
                        description: "Sub-10ns state reads with atomic capsules",
                    },
                    // ... more features
                ]
            />
            <Pricing />
            <CallToAction
                headline="Ready to Build?"
                subheadline="Start your free trial today"
                cta="Get Started"
            />
        </>
    }
}
```

### Pattern 2: Responsive Grid

```rust
view! {
    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-8">
        <FeatureCard icon="zap" title="Fast" description="..." />
        <FeatureCard icon="shield" title="Safe" description="..." />
        <FeatureCard icon="box" title="Small" description="..." />
    </div>
}
```

### Pattern 3: Conditional Rendering

```rust
view! {
    <Show when=move || loading.get() fallback=|| view! { <Content /> }>
        <Spinner size="large" color="primary" />
    </Show>
}
```

---

## Styling System

### Byzantine Purple Design System

**Color Palette**:
- **Primary**: #4B0082 (Byzantine Purple)
- **Secondary**: #6A00B8 (Byzantine Medium)
- **Accent**: #FFD700 (Gold)
- **Background**: #0A0A0F (Dark Blue-Black)
- **Text**: #FFFFFF (White), #E5E7EB (Light Gray)

**Spacing Scale** (8px base):
- **xs**: 4px
- **sm**: 8px
- **md**: 16px
- **lg**: 24px
- **xl**: 32px
- **2xl**: 48px
- **3xl**: 64px

**Typography**:
- **Font Family**: Inter, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif
- **Font Sizes**: 12px, 14px, 16px, 20px, 28px, 36px, 48px
- **Font Weights**: 400 (normal), 500 (medium), 600 (semibold), 700 (bold)

---

## Accessibility

### WCAG 2.1 AA Compliance

**Color Contrast**:
- ✅ 4.5:1 minimum for normal text
- ✅ 3:1 minimum for large text (18px+)
- ✅ Tested with WAVE, axe DevTools

**Keyboard Navigation**:
- ✅ All interactive elements focusable
- ✅ Tab order follows visual order
- ✅ Enter/Space key activation for buttons
- ✅ Escape key closes modals

**Screen Reader Support**:
- ✅ ARIA labels for icon-only buttons
- ✅ `aria-live` regions for dynamic content
- ✅ Semantic HTML (`<nav>`, `<main>`, `<article>`)
- ✅ Alt text for all images

**Focus Indicators**:
- ✅ Visible focus ring (2px solid Byzantine Purple)
- ✅ Skip-to-content link

---

## Development

### Adding a New Component

```bash
# 1. Create component file
touch src/components/common/my_component.rs

# 2. Implement component
# (See example in README.md)

# 3. Export in mod.rs
echo "pub mod my_component;" >> src/components/common/mod.rs
echo "pub use my_component::MyComponent;" >> src/components/common/mod.rs

# 4. Document in this file (COMPONENTS.md)

# 5. Add tests
# (Unit tests in tests/unit_components.rs)

# 6. Test
cargo test
trunk serve
```

---

**Last Updated**: 2025-10-18
**Maintainer**: kindly.ai Team
**License**: MIT OR Apache-2.0
