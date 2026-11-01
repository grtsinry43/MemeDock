import { LazyMotion, MotionConfig, domMax } from 'motion/react'
import { ThemeProvider } from './lib/theme'
import { I18nProvider, useI18n } from './lib/i18n'
import { Nav } from './components/layout/Nav'
import { Footer } from './components/layout/Footer'
import { ScrollProgress } from './components/ui/ScrollProgress'
import { Hero } from './sections/Hero'
import { Manifesto } from './sections/Manifesto'
import { Collect } from './sections/Collect'
import { Organize } from './sections/Organize'
import { Share } from './sections/Share'
import { Keep } from './sections/Keep'
import { SyncSection } from './sections/SyncSection'
import { Native } from './sections/Native'
import { Faq } from './sections/Faq'
import { Download } from './sections/Download'

export default function App() {
  return (
    <ThemeProvider>
      <I18nProvider>
        <MotionConfig reducedMotion="user">
          <LazyMotion features={domMax} strict>
            <Page />
          </LazyMotion>
        </MotionConfig>
      </I18nProvider>
    </ThemeProvider>
  )
}

function Page() {
  const { t } = useI18n()

  return (
    <>
      <a href="#main" className="skip-link">
        {t.nav.skip}
      </a>
      <ScrollProgress />
      <div id="top" />
      <Nav />
      <main id="main">
        <Hero />
        <Manifesto />
        <Collect />
        <Organize />
        <Share />
        <Keep />
        <SyncSection />
        <Native />
        <Faq />
        <Download />
      </main>
      <Footer />
    </>
  )
}
