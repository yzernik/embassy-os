import { NgModule } from '@angular/core'
import { CommonModule } from '@angular/common'
import { InstallWizardComponent } from './install-wizard.component'
import { IonicModule } from '@ionic/angular'
import { RouterModule } from '@angular/router'
import { EmverPipesModule } from '@start9labs/shared'
import { DependentsComponentModule } from './dependents/dependents.component.module'
import { CompleteComponentModule } from './complete/complete.component.module'
import { NotesComponentModule } from './notes/notes.component.module'
import { AlertComponentModule } from './alert/alert.component.module'

@NgModule({
  declarations: [InstallWizardComponent],
  imports: [
    CommonModule,
    IonicModule,
    RouterModule.forChild([]),
    EmverPipesModule,
    DependentsComponentModule,
    CompleteComponentModule,
    NotesComponentModule,
    AlertComponentModule,
  ],
  exports: [InstallWizardComponent],
})
export class InstallWizardComponentModule {}
