// Smooth Scrolling
function scrollToSection(sectionId) {
  const section = document.getElementById(sectionId);
  const headerHeight = document.querySelector('header').offsetHeight;
  window.scrollTo({
    top: section.offsetTop - headerHeight,
    behavior: 'smooth'
  });
}

// Intersection Observer for Animations
const sections = document.querySelectorAll('.section');

const observer = new IntersectionObserver((entries) => {
  entries.forEach(entry => {
    if (entry.isIntersecting) {
      entry.target.classList.add('visible');
    }
  });
}, { threshold: 0.1 });

sections.forEach(section => {
  observer.observe(section);
});