// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import starlightThemeRapide from 'starlight-theme-rapide';
import vercel from '@astrojs/vercel';

const site = 'https://hegel.dev';
// Link embed (Discord etc) thumbnail. Must be referenced by absolute URL.
const ogImage = `${site}/hedgel-opengraph.jpg`;

// https://astro.build/config
export default defineConfig({
    site,
    markdown: {
		// Liam: disable smart quotes, among other "features" I'm not a fan
		// of: https://daringfireball.net/projects/smartypants/
        smartypants: false,
    },
    trailingSlash: 'never',
    // Poll for file changes in the dev server rather than relying on macOS
    // FSEvents, which can silently stop delivering events and leave the dev
    // server serving stale content.
    vite: { server: { watch: { usePolling: true, interval: 300 } } },
    // The protocol reference documented the retired hegel-core wire protocol,
    // and the libhegel reference duplicated the C header. Keep the old URLs
    // alive by pointing them at the header itself.
    redirects: {
        '/reference/protocol': 'https://github.com/hegeldev/hegel-rust/blob/main/hegel-c/include/hegel.h',
        '/reference/libhegel': 'https://github.com/hegeldev/hegel-rust/blob/main/hegel-c/include/hegel.h',
    },
	integrations: [
		starlight({
			title: 'Hegel',
			description: 'A universal property-based testing engine and family of libraries, built on Hypothesis',
			favicon: '/favicon.ico',
			head: [
				{ tag: 'link', attrs: { rel: 'icon', type: 'image/png', sizes: '96x96', href: '/favicon-96x96.png' } },
				{ tag: 'link', attrs: { rel: 'apple-touch-icon', sizes: '180x180', href: '/apple-touch-icon.png' } },
				{ tag: 'meta', attrs: { name: 'twitter:card', content: 'summary' } },
				{ tag: 'meta', attrs: { property: 'og:image', content: ogImage } },
				{ tag: 'meta', attrs: { property: 'og:image:width', content: '512' } },
				{ tag: 'meta', attrs: { property: 'og:image:height', content: '512' } },
				{ tag: 'meta', attrs: { property: 'og:image:alt', content: 'Hedgel, the Hegel mascot.' } },
				{ tag: 'meta', attrs: { name: 'twitter:image', content: ogImage } },
				{ tag: 'meta', attrs: { name: 'theme-color', content: '#8b6f4e' } },
			],
			social: [
				{ icon: 'github', label: 'GitHub', href: 'https://github.com/hegeldev' },
				{ icon: 'discord', label: 'Discord', href: 'https://discord.gg/yeggwH7vf2' },
			],
			sidebar: [
				{
					label: 'Introduction',
					items: [
						{ slug: 'intro/getting-started' },
					],
				},
				{
					label: 'Libraries',
					items: [
						{ label: 'hegel-rust', link: 'https://github.com/hegeldev/hegel-rust', attrs: { target: '_blank', class: 'external-link' } },
						{ label: 'hegel-go', link: 'https://github.com/hegeldev/hegel-go', attrs: { target: '_blank', class: 'external-link' } },
						{ label: 'hegel-cpp', link: 'https://github.com/hegeldev/hegel-cpp', attrs: { target: '_blank', class: 'external-link' } },
						{ label: 'hegel-typescript', link: 'https://github.com/hegeldev/hegel-typescript', attrs: { target: '_blank', class: 'external-link' } },
						{ label: 'hegel-java', link: 'https://github.com/hegeldev/hegel-java', attrs: { target: '_blank', class: 'external-link' } },
						{ label: 'hegel-ocaml', link: 'https://github.com/hegeldev/hegel-ocaml', attrs: { target: '_blank', class: 'external-link' } },
					],
				},
				// re-enable once we actually have content here!
				//
				// {
				// 	label: 'How-to guides',
				// 	autogenerate: { directory: 'how-to' },
				// },
				{
					label: 'Explanation',
					items: [
						{ slug: 'explanation/how-hegel-works' },
						{ slug: 'explanation/why-hegel' },
					],
				},
				{
					label: 'API documentation',
					items: [
						{ label: 'hegel-rust', link: 'https://docs.rs/hegeltest', attrs: { target: '_blank', class: 'external-link' } },
						{ label: 'hegel-go', link: 'https://pkg.go.dev/hegel.dev/go/hegel', attrs: { target: '_blank', class: 'external-link' } },
						{ label: 'hegel-cpp', link: '/cpp/', attrs: { target: '_blank', class: 'external-link' } },
						{ label: 'hegel-typescript', link: '/typescript/', attrs: { target: '_blank', class: 'external-link' } },
						{ label: 'hegel-java', link: 'https://javadoc.io/doc/dev.hegel/hegel', attrs: { target: '_blank', class: 'external-link' } },
						{ label: 'hegel-ocaml', link: '/ocaml/', attrs: { target: '_blank', class: 'external-link' } },
					],
				},
				{
					slug: 'community',
				},
				{
					slug: 'compatibility',
				},
			],
            customCss: ['./src/styles/style.css'],
            plugins: [ starlightThemeRapide() ],
            // Preserve literal tab characters in our code blocks so that our
            // css rule for tab sizes can collapse them on mobile. Default is 2
            expressiveCode: { tabWidth: 0 },
		}),
	],
    adapter: vercel({
        webAnalytics: { enabled: true },
    }),
});
